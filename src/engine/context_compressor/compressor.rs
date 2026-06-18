use super::*;

impl ContextCompressor {
    pub fn new(max_context_tokens: u64) -> Self {
        Self {
            budget: TokenBudget::new(max_context_tokens),
            token_estimate_profile: TokenEstimateProfile::GeneralText,
            time_config: TimeBasedConfig::default(),
            session_start: std::time::Instant::now(),
            accumulated_summary: None,
            compression_count: 0,
            last_failure_time: None,
            cooldown_secs: 600, // 10 分钟冷却
            llm_provider: None,
            llm_model: String::new(),
            llm_summary_stable_prefix: None,
            total_tokens_before: 0,
            total_tokens_after: 0,
            llm_compression_attempts: 0,
            llm_compression_failures: 0,
            consecutive_llm_failures: 0,
            max_consecutive_llm_failures: 3,
            compact_sequence: 0,
            compact_metadata_history: Vec::new(),
            compaction_records: Vec::new(),
            compaction_attempt_records: Vec::new(),
            consecutive_compaction_failures: 0,
            consecutive_no_gain_compactions: 0,
            max_consecutive_compaction_failures: 2,
            max_consecutive_no_gain_compactions: 2,
            // 策略：压缩摘要中永远保留技能提醒 marker。
            // 如果未来希望按真实技能状态决定是否保留，需要接入 SkillRuntime。
            preserve_skills_marker: true,
        }
    }

    pub fn from_model_context_profile(
        profile: &crate::engine::model_context::ModelContextProfile,
    ) -> Self {
        Self {
            budget: TokenBudget::from_model_context_profile(profile),
            token_estimate_profile: TokenEstimateProfile::for_model_context(profile),
            ..Self::new(profile.context_window_tokens)
        }
    }

    pub fn estimate_messages_tokens(&self, messages: &[Message]) -> u64 {
        estimate_messages_tokens_for_profile(messages, self.token_estimate_profile)
    }

    pub fn token_counter_label(&self) -> &'static str {
        self.token_estimate_profile.source_label()
    }

    /// 获取当前压缩警告级别
    pub fn warning_level(&self, messages: &[Message]) -> CompressionWarning {
        let tokens = self.estimate_messages_tokens(messages);
        let total = tokens + self.budget.system_prompt_tokens + self.budget.tool_schemas_tokens;
        let ratio = total as f64 / self.budget.max_context_tokens as f64;
        CompressionWarning::from_usage_ratio(ratio)
    }

    fn token_pressure_for_tokens(&self, message_tokens: u64) -> ContextTokenPressure {
        let total = message_tokens
            .saturating_add(self.budget.system_prompt_tokens)
            .saturating_add(self.budget.tool_schemas_tokens);
        let ratio = if self.budget.max_context_tokens == 0 {
            1.0
        } else {
            total as f64 / self.budget.max_context_tokens as f64
        };
        ContextTokenPressure::from_usage_ratio(ratio)
    }

    /// 检查是否需要基于时间的压缩
    pub fn needs_time_based_compression(&self, messages: &[Message]) -> bool {
        if !self.time_config.enabled {
            return false;
        }
        let elapsed = self.session_start.elapsed().as_secs();
        let msg_count = messages.len();

        elapsed > self.time_config.session_duration_threshold_secs
            || msg_count > self.time_config.message_count_threshold
    }

    /// 微压缩：轻量级压缩，不触发 LLM，仅裁剪工具输出
    /// 用于中等长度对话或空闲后轻量整理
    pub fn micro_compress(&mut self, messages: &[Message]) -> Vec<Message> {
        self.micro_compress_with_strategy(
            messages,
            ContextCompactionStrategy::MicroCompact,
            Some(CompressionLevel::Light),
        )
    }

    /// Snip old tool outputs without summarizing the conversation.
    pub fn snip_tool_results(&mut self, messages: &[Message]) -> Vec<Message> {
        let (result, tokens_before, tokens_after) = self.snip_tool_results_candidate(messages);
        self.record_tool_snip_compaction(messages.len(), result.len(), tokens_before, tokens_after);
        result
    }

    /// Snip old tool outputs only when it reduces estimated tokens.
    pub fn snip_tool_results_if_reduces(&mut self, messages: &[Message]) -> Option<Vec<Message>> {
        let (result, tokens_before, tokens_after) = self.snip_tool_results_candidate(messages);
        if tokens_after >= tokens_before {
            return None;
        }
        self.record_tool_snip_compaction(messages.len(), result.len(), tokens_before, tokens_after);
        Some(result)
    }

    fn snip_tool_results_candidate(&self, messages: &[Message]) -> (Vec<Message>, u64, u64) {
        let tokens_before = self.estimate_messages_tokens(messages);
        let result = Self::prune_old_tool_results(messages);
        let tokens_after = self.estimate_messages_tokens(&result);
        (result, tokens_before, tokens_after)
    }

    fn record_tool_snip_compaction(
        &mut self,
        messages_before: usize,
        messages_after: usize,
        tokens_before: u64,
        tokens_after: u64,
    ) {
        self.total_tokens_before += tokens_before;
        self.total_tokens_after += tokens_after;
        self.record_compaction(CompactionRuntimeRecord {
            strategy: ContextCompactionStrategy::Snip,
            level: None,
            trigger: None,
            token_pressure: Some(self.token_pressure_for_tokens(tokens_before)),
            messages_before,
            messages_after,
            tokens_before,
            tokens_after,
            token_delta: compaction_token_delta(tokens_before, tokens_after),
            stage_order: compaction_stage_order(ContextCompactionStrategy::Snip),
            boundary_id: None,
            sequence: None,
            preserved_tail_count: None,
            retained_items: vec!["recent_tool_results:last_3".to_string()],
            provenance: vec!["tool_result_snip".to_string()],
        });
    }

    fn micro_compress_with_strategy(
        &mut self,
        messages: &[Message],
        strategy: ContextCompactionStrategy,
        level: Option<CompressionLevel>,
    ) -> Vec<Message> {
        let tokens_before = self.estimate_messages_tokens(messages);
        self.total_tokens_before += tokens_before;

        // 只做 Phase 0（裁剪旧工具输出）和 Phase 5（工具对校验）
        let pruned = Self::prune_old_tool_results(messages);
        let result = Self::sanitize_tool_pairs(pruned);

        let tokens_after = self.estimate_messages_tokens(&result);
        self.total_tokens_after += tokens_after;
        self.record_compaction(CompactionRuntimeRecord {
            strategy,
            level: level.map(|value| value.label().to_string()),
            trigger: None,
            token_pressure: Some(self.token_pressure_for_tokens(tokens_before)),
            messages_before: messages.len(),
            messages_after: result.len(),
            tokens_before,
            tokens_after,
            token_delta: compaction_token_delta(tokens_before, tokens_after),
            stage_order: compaction_stage_order(strategy),
            boundary_id: None,
            sequence: None,
            preserved_tail_count: None,
            retained_items: vec![
                "recent_tool_results:last_3".to_string(),
                "tool_call_pairs:sanitized".to_string(),
            ],
            provenance: vec![
                "tool_result_snip".to_string(),
                "tool_pair_sanitize".to_string(),
            ],
        });

        info!(
            "Micro compression: {} messages -> {} messages ({} -> {} tokens)",
            messages.len(),
            result.len(),
            tokens_before,
            tokens_after
        );
        result
    }

    /// 设置系统 prompt 预估大小
    pub fn with_system_prompt_tokens(mut self, tokens: u64) -> Self {
        self.budget.system_prompt_tokens = tokens;
        self
    }

    /// 设置工具 schema 预估大小
    pub fn with_tool_schemas_tokens(mut self, tokens: u64) -> Self {
        self.budget.tool_schemas_tokens = tokens;
        self
    }

    /// 设置 LLM Provider（用于高质量摘要生成）
    pub fn with_llm_provider(
        mut self,
        provider: std::sync::Arc<dyn crate::services::api::LlmProvider>,
        model: impl Into<String>,
    ) -> Self {
        self.llm_provider = Some(provider);
        self.llm_model = model.into();
        self
    }

    pub fn set_llm_summary_stable_prefix(&mut self, prefix: impl Into<String>) {
        let prefix = prefix.into();
        if prefix.trim().is_empty() {
            self.llm_summary_stable_prefix = None;
        } else {
            self.llm_summary_stable_prefix = Some(prefix);
        }
    }

    pub fn set_llm_summary_stable_prefix_from_messages(&mut self, messages: &[Message]) {
        if let Some(prefix) = messages.iter().find_map(|message| match message {
            Message::System { content }
                if !content.trim().is_empty()
                    && !crate::engine::cache_stability::is_dynamic_context_system_message(
                        content,
                    ) =>
            {
                Some(content.clone())
            }
            _ => None,
        }) {
            self.llm_summary_stable_prefix = Some(prefix);
        }
    }

    /// 检查是否在冷却期（压缩失败后）
    pub fn is_in_cooldown(&self) -> bool {
        if let Some(last_failure) = self.last_failure_time {
            last_failure.elapsed().as_secs() < self.cooldown_secs
        } else {
            false
        }
    }

    /// 前置检查：是否需要压缩（包括系统提示和工具 schema）
    pub fn preflight_check(
        &self,
        messages: &[Message],
        system_prompt_tokens: u64,
        tool_schemas_tokens: u64,
    ) -> bool {
        if self.is_in_cooldown() {
            return false;
        }
        let total = self
            .estimate_messages_tokens(messages)
            .saturating_add(system_prompt_tokens)
            .saturating_add(tool_schemas_tokens);
        let threshold = self.budget.max_context_tokens * 80 / 100;
        total > threshold
    }

    /// 检查是否需要压缩
    pub fn needs_compression(&self, messages: &[Message]) -> bool {
        if self.is_in_cooldown() {
            return false;
        }
        let tokens = self.estimate_messages_tokens(messages);
        self.budget.needs_compression(tokens)
    }

    /// 按级别压缩消息列表
    pub fn compress_with_level(
        &mut self,
        messages: &[Message],
        level: CompressionLevel,
    ) -> Vec<Message> {
        self.compress_with_level_for_strategy(
            messages,
            level,
            ContextCompactionStrategy::AutoCompact,
        )
    }

    fn compress_with_level_for_strategy(
        &mut self,
        messages: &[Message],
        level: CompressionLevel,
        strategy: ContextCompactionStrategy,
    ) -> Vec<Message> {
        let tokens_before = self.estimate_messages_tokens(messages);

        match level {
            CompressionLevel::None => {
                self.record_compaction(CompactionRuntimeRecord {
                    strategy,
                    level: Some(level.label().to_string()),
                    trigger: None,
                    token_pressure: Some(self.token_pressure_for_tokens(tokens_before)),
                    messages_before: messages.len(),
                    messages_after: messages.len(),
                    tokens_before,
                    tokens_after: tokens_before,
                    token_delta: 0,
                    stage_order: compaction_stage_order(ContextCompactionStrategy::NoOp),
                    boundary_id: None,
                    sequence: None,
                    preserved_tail_count: None,
                    retained_items: vec!["messages:all".to_string()],
                    provenance: vec!["level:none".to_string()],
                });
                messages.to_vec()
            }
            CompressionLevel::Light => {
                let r = self.micro_compress_with_strategy(messages, strategy, Some(level));
                let tokens_after = self.estimate_messages_tokens(&r);
                info!(
                    "Light compression ({}): {} -> {} tokens",
                    level.label(),
                    tokens_before,
                    tokens_after
                );
                r
            }
            CompressionLevel::Medium => {
                let r =
                    self.compress_with_summary_for_strategy(messages, None, strategy, Some(level));
                let tokens_after = self.estimate_messages_tokens(&r);
                info!(
                    "Medium compression ({}): {} -> {} tokens",
                    level.label(),
                    tokens_before,
                    tokens_after
                );
                r
            }
            CompressionLevel::Heavy => {
                // Heavy 需要 LLM，在 compress_async 中处理
                self.compress_with_summary_for_strategy(messages, None, strategy, Some(level))
            }
        }
    }

    /// 异步压缩消息列表（分层压缩流水线）
    /// 根据 token 使用率自动选择压缩级别：
    /// - Light (<70%): 只裁剪工具输出
    /// - Medium (70-85%): 裁剪 + 启发式摘要
    /// - Heavy (>85%): 裁剪 + LLM 摘要
    pub async fn compress_async(&mut self, messages: &[Message]) -> Vec<Message> {
        self.compress_async_with_strategy(messages, ContextCompactionStrategy::AutoCompact)
            .await
    }

    pub async fn compress_async_with_strategy(
        &mut self,
        messages: &[Message],
        strategy: ContextCompactionStrategy,
    ) -> Vec<Message> {
        let tokens_before = self.estimate_messages_tokens(messages);
        let total =
            tokens_before + self.budget.system_prompt_tokens + self.budget.tool_schemas_tokens;
        let usage_ratio = total as f64 / self.budget.max_context_tokens as f64;

        // ── Economic guard: skip expensive compression for short conversations ──
        // Borrowed from Reasonix: don't pay LLM summarization cost when the
        // conversation is too short to benefit. Snip-only is free and sufficient.
        let message_count = messages.len();
        let is_short_conversation = message_count < 20;
        let estimated_summary_savings = if is_short_conversation {
            0.0 // Short convos gain little from summarization
        } else {
            (tokens_before as f64 * 0.3).min(8000.0) // Rough estimate: 30% of body
        };
        let skip_heavy = is_short_conversation && estimated_summary_savings < 2000.0;

        // ── Runtime diet integration: avoid re-compressing when recent
        //     compressions produced no gains. ──
        let recent_no_gain = self.consecutive_no_gain_compactions >= 2;
        if recent_no_gain && skip_heavy {
            debug!(
                "Skipping compression: {} consecutive no-gain compactions, short conversation",
                self.consecutive_no_gain_compactions
            );
            return messages.to_vec();
        }
        // ── End economic guard ──

        let level = if skip_heavy {
            // Force Medium at most — skip LLM compression for short convos
            CompressionLevel::Medium
        } else {
            CompressionLevel::auto_select(
                usage_ratio,
                self.compression_count,
                self.consecutive_llm_failures,
                self.has_llm_provider(),
            )
        };

        debug!(
            "Compression auto-selected level={} (usage={:.1}%, count={}, llm_failures={})",
            level.label(),
            usage_ratio * 100.0,
            self.compression_count,
            self.consecutive_llm_failures
        );

        // Light/Medium 不需要 LLM，直接同步处理
        if level == CompressionLevel::Light || level == CompressionLevel::Medium {
            return self.compress_with_level_for_strategy(messages, level, strategy);
        }

        // Heavy: 尝试 LLM 摘要
        if self.has_llm_provider()
            && !self.is_in_cooldown()
            && self.consecutive_llm_failures < self.max_consecutive_llm_failures
        {
            self.llm_compression_attempts += 1;
            let prev_summary = self.accumulated_summary.as_ref().map(|s| s.to_text());
            match self
                .llm_summarize_middle(messages, prev_summary.as_deref())
                .await
            {
                Some(summary_text) => {
                    self.consecutive_llm_failures = 0;
                    let compressed = self.compress_with_summary_for_strategy(
                        messages,
                        Some(&summary_text),
                        strategy,
                        Some(level),
                    );
                    let tokens_after = self.estimate_messages_tokens(&compressed);
                    info!(
                        "Heavy (LLM) compression succeeded: {} -> {} tokens (saved {}%)",
                        tokens_before,
                        tokens_after,
                        if tokens_before > 0 {
                            (tokens_before - tokens_after) * 100 / tokens_before
                        } else {
                            0
                        }
                    );
                    compressed
                }
                None => {
                    self.llm_compression_failures += 1;
                    self.consecutive_llm_failures += 1;
                    self.record_failure();
                    let compressed = self.compress_with_summary_for_strategy(
                        messages,
                        None,
                        strategy,
                        Some(level),
                    );
                    let tokens_after = self.estimate_messages_tokens(&compressed);
                    warn!(
                        "LLM compression failed, fell back to medium: {} -> {} tokens",
                        tokens_before, tokens_after
                    );
                    compressed
                }
            }
        } else {
            if self.consecutive_llm_failures >= self.max_consecutive_llm_failures {
                warn!(
                    "LLM compression disabled after {} consecutive failures; using medium compression.",
                    self.consecutive_llm_failures
                );
            }
            self.compress_with_summary_for_strategy(messages, None, strategy, Some(level))
        }
    }

    /// 压缩消息列表
    /// 返回压缩后的消息列表
    pub fn compress(&mut self, messages: &[Message]) -> Vec<Message> {
        self.compress_with_summary(messages, None)
    }

    /// 使用预计算的摘要文本压缩（同步）
    /// summary_text: Some(text) = 使用 LLM 生成的摘要; None = 使用启发式
    pub fn compress_with_summary(
        &mut self,
        messages: &[Message],
        summary_text: Option<&str>,
    ) -> Vec<Message> {
        self.compress_with_summary_for_strategy(
            messages,
            summary_text,
            ContextCompactionStrategy::AutoCompact,
            None,
        )
    }

    fn compress_with_summary_for_strategy(
        &mut self,
        messages: &[Message],
        summary_text: Option<&str>,
        strategy: ContextCompactionStrategy,
        level: Option<CompressionLevel>,
    ) -> Vec<Message> {
        let original_message_count = messages.len();
        let original_tokens_before = self.estimate_messages_tokens(messages);
        let summary_source_tag = if summary_text.is_some() {
            "summary_source:llm"
        } else {
            "summary_source:heuristic"
        };
        if messages.is_empty() {
            return messages.to_vec();
        }
        self.total_tokens_before += original_tokens_before;
        let session_memory = SessionMemoryCompact::analyze(messages);
        let runtime_continuity = RuntimeContinuityFacts::analyze(messages);

        info!(
            "Compressing {} messages (budget: {} available tokens, iteration: {})",
            messages.len(),
            self.budget.available_for_history(),
            self.compression_count + 1
        );

        // Phase 0: 预处理 — 裁剪旧工具输出（廉价，不需要 LLM）
        let messages = Self::prune_old_tool_results(messages);

        // Phase 1: 保护头部（system prompt）
        let (head, rest) = self.split_head(&messages);

        // Phase 2: 正向边界对齐 — 跳过头部之后的孤立 tool results
        let head_len = head.len();
        let aligned_start = Self::align_boundary_forward(rest, 0);
        let rest = &rest[aligned_start..];
        let head = &messages[..head_len + aligned_start];

        // Phase 3: 保护尾部（按 token 预算，soft_ceiling 防超大消息切割）
        let (middle, tail) = self.split_tail(rest);

        // Phase 3: 对中间部分生成摘要
        let mut summary_text = if let Some(text) = summary_text {
            // 使用 LLM 预计算的摘要
            let new_summary = StructuredSummary::from_text(text);
            if let Some(ref mut acc) = self.accumulated_summary {
                acc.merge(&new_summary);
                acc.to_text()
            } else {
                self.accumulated_summary = Some(new_summary.clone());
                new_summary.to_text()
            }
        } else {
            // 启发式摘要
            self.summarize_middle(middle)
        };
        session_memory.inject_into_summary(&mut summary_text);
        runtime_continuity.inject_into_summary(&mut summary_text);

        // Preserve active skills through compression (Reasonix skill-pin pattern).
        // Skills loaded by the agent are embedded in the system prompt pre-compression;
        // this marker tells the model those definitions are still active.
        if self.preserve_skills_marker() {
            summary_text.push_str("\n\n");
            summary_text.push_str(PRESERVED_SKILLS_MARKER);
        }

        // Phase 4: 组装结果
        let mut result = head.to_vec();

        // 生成 Compact Boundary 元数据（在 summary 组装前准备）
        let compact_meta = if !summary_text.is_empty() {
            self.compact_sequence += 1;
            Some(CompactMetadata {
                sequence: self.compact_sequence,
                boundary_id: format!("cb-{}", Uuid::new_v4().simple()),
                preserved_tail_count: tail.len(),
                messages_before: original_message_count,
                messages_after: head.len() + tail.len() + 1, // +1 for summary
                tokens_before: original_tokens_before,
                tokens_after: 0, // 将在后面更新
                timestamp: chrono::Local::now().to_rfc3339(),
            })
        } else {
            None
        };

        if !summary_text.is_empty() {
            let mut formatted_summary = if self.compression_count > 0 {
                format!(
                    "{}\n（上下文已压缩 {} 次，保留累积知识）\n\n{}",
                    SUMMARY_PREFIX,
                    self.compression_count + 1,
                    summary_text
                )
            } else {
                format!("{}\n\n{}", SUMMARY_PREFIX, summary_text)
            };

            // 嵌入 Compact Boundary 标记
            if let Some(ref meta) = compact_meta {
                formatted_summary.push_str(&meta.to_boundary_marker());
            }

            // ── 消息角色交替（Hermes 风格）──
            // OpenAI API 要求消息角色交替，不能连续两个相同角色
            // 检查 head 最后一条和 tail 第一条的角色，选择合适的摘要角色
            let last_head_role = head
                .last()
                .map(|m| match m {
                    Message::System { .. } => "system",
                    Message::User { .. } => "user",
                    Message::Assistant { .. } => "assistant",
                    Message::Tool { .. } => "tool",
                })
                .unwrap_or("system");

            let first_tail_role = if tail.is_empty() {
                "none"
            } else {
                match &tail[0] {
                    Message::System { .. } => "system",
                    Message::User { .. } => "user",
                    Message::Assistant { .. } => "assistant",
                    Message::Tool { .. } => "tool",
                }
            };

            // 选择摘要消息的角色（优先避免与 head 碰撞）
            let summary_role = if last_head_role == "user" || last_head_role == "tool" {
                "assistant"
            } else {
                "user"
            };

            // 如果选择的角色与 tail 碰撞，且翻转不会与 head 碰撞，翻转
            let summary_role = if summary_role == first_tail_role {
                let flipped = if summary_role == "user" {
                    "assistant"
                } else {
                    "user"
                };
                if flipped != last_head_role {
                    flipped
                } else {
                    // 两个角色都会产生连续相同角色
                    // 将摘要合并到第一条 tail 消息中
                    "merge"
                }
            } else {
                summary_role
            };

            if summary_role == "merge" && !tail.is_empty() {
                // 合并模式：将摘要前置到第一条 tail 消息
                let mut merged_tail = tail.to_vec();
                let original_content = merged_tail[0].content();
                merged_tail[0] = match &merged_tail[0] {
                    Message::User { .. } => {
                        Message::user(format!("{}\n\n{}", formatted_summary, original_content))
                    }
                    Message::Assistant { content: _, .. } => {
                        // 需要保留 tool_calls
                        // 这里简化处理，直接用 user 消息
                        Message::user(format!("{}\n\n{}", formatted_summary, original_content))
                    }
                    _ => Message::user(format!("{}\n\n{}", formatted_summary, original_content)),
                };
                result.extend_from_slice(&merged_tail);
            } else {
                let summary_msg = match summary_role {
                    "assistant" => Message::assistant(&formatted_summary),
                    _ => Message::system(&formatted_summary),
                };
                result.push(summary_msg);
                result.extend_from_slice(tail);
            }
        } else {
            result.extend_from_slice(tail);
        }

        // Phase 5: 校验工具调用对完整性（移除孤立 tool result + 插入 stub）
        let result = Self::sanitize_tool_pairs(result);

        // 更新 compact metadata 的 tokens_after 并保存到历史
        let tokens_after = self.estimate_messages_tokens(&result);
        let mut recorded_meta = None;
        if let Some(mut meta) = compact_meta {
            meta.tokens_after = tokens_after;
            recorded_meta = Some(meta.clone());
            self.compact_metadata_history.push(meta);
        }

        self.total_tokens_after += tokens_after;
        let mut provenance = vec![
            format!(
                "level:{}",
                level.map(|value| value.label()).unwrap_or("summary")
            ),
            "summary:structured".to_string(),
            "tool_result_snip".to_string(),
            "tool_pair_sanitize".to_string(),
        ];
        if summary_text.contains("Frequently Accessed Files")
            || summary_text.contains("Pending Tasks")
            || summary_text.contains("Common Tool Patterns")
            || summary_text.contains("User Preferences")
        {
            provenance.push("summary_memory:session".to_string());
        }
        if !runtime_continuity.is_empty() {
            provenance.push("summary_memory:runtime_continuity".to_string());
        }
        provenance.push(if summary_text.is_empty() {
            "summary_source:empty".to_string()
        } else {
            summary_source_tag.to_string()
        });
        provenance.extend(session_memory.provenance_tags());
        provenance.extend(runtime_continuity.provenance_tags());
        self.record_compaction(CompactionRuntimeRecord {
            strategy,
            level: level.map(|value| value.label().to_string()),
            trigger: None,
            token_pressure: Some(self.token_pressure_for_tokens(original_tokens_before)),
            messages_before: original_message_count,
            messages_after: result.len(),
            tokens_before: original_tokens_before,
            tokens_after,
            token_delta: compaction_token_delta(original_tokens_before, tokens_after),
            stage_order: compaction_stage_order(strategy),
            boundary_id: recorded_meta.as_ref().map(|meta| meta.boundary_id.clone()),
            sequence: recorded_meta.as_ref().map(|meta| meta.sequence),
            preserved_tail_count: recorded_meta.as_ref().map(|meta| meta.preserved_tail_count),
            retained_items: compaction_retained_items(
                head.len(),
                tail.len(),
                recorded_meta.as_ref(),
                &session_memory,
                &runtime_continuity,
            ),
            provenance,
        });
        self.compression_count += 1;

        info!(
            "Compressed to {} messages (compact_boundary #{})",
            result.len(),
            self.compact_sequence
        );
        result
    }

    /// 预处理：裁剪旧工具输出（替换为简短摘要）
    pub(super) fn prune_old_tool_results(messages: &[Message]) -> Vec<Message> {
        let mut result = Vec::with_capacity(messages.len());
        // 保留最近 3 轮的工具输出，更早的裁剪
        let tool_msg_count = messages
            .iter()
            .filter(|m| matches!(m, Message::Tool { .. }))
            .count();
        let keep_last_n = 3;
        let mut tool_seen = 0;

        for msg in messages {
            match msg {
                Message::Tool {
                    tool_call_id,
                    content,
                } => {
                    tool_seen += 1;
                    let is_recent = tool_seen > tool_msg_count.saturating_sub(keep_last_n);
                    if is_recent
                        || tool_msg_count <= keep_last_n
                        || Self::is_protected_tool_output(content)
                    {
                        result.push(msg.clone());
                    } else {
                        let keep_len = if Self::is_critical_tool_output(content) {
                            1000
                        } else {
                            200
                        };
                        // 裁剪：关键失败链路保留更多上下文，普通结果保留短摘要
                        let truncated = if content.len() > keep_len {
                            let safe: String = content.chars().take(keep_len).collect();
                            format!("{}...(truncated)", safe)
                        } else {
                            content.clone()
                        };
                        result.push(Message::Tool {
                            tool_call_id: tool_call_id.clone(),
                            content: truncated,
                        });
                    }
                }
                _ => result.push(msg.clone()),
            }
        }
        result
    }

    fn is_protected_tool_output(content: &str) -> bool {
        let lower = content.to_ascii_lowercase();
        content.contains("[exit status:")
            || content.contains("required command")
            || lower.contains("cargo test")
            || lower.contains("cargo check")
            || lower.contains("cargo build")
            || (lower.contains("rg ") && lower.contains("fixtures/"))
            || lower.contains("required validation:")
            || lower.contains("permission_decision_evidence")
            || lower.contains("permission decision:")
            || lower.contains("permission denied")
            || (lower.contains("permission") && lower.contains("risk_level"))
            || (lower.contains("permission") && lower.contains("matched_rules"))
            || lower.contains("checkpoint")
            || lower.contains("failure_owner")
            || lower.contains("failure owner")
            || lower.contains("[preserved skills")
            || lower.contains("preserved skills")
            || lower.contains("active skill")
    }

    fn is_critical_tool_output(content: &str) -> bool {
        let lower = content.to_lowercase();
        let critical_markers = [
            "result: error",
            "error",
            "failed",
            "panic",
            "traceback",
            "diagnostic",
            "assertion",
            "permission denied",
            "cannot find",
            "no such file",
            "test failed",
        ];
        critical_markers.iter().any(|m| lower.contains(m))
    }

    /// 分离头部（system prompt）
    fn split_head<'a>(&self, messages: &'a [Message]) -> (&'a [Message], &'a [Message]) {
        let head_end = messages
            .iter()
            .position(|m| !matches!(m, Message::System { .. }))
            .unwrap_or(messages.len());
        (&messages[..head_end], &messages[head_end..])
    }

    /// 正向边界对齐：如果 compress_start 落在孤立的 tool result 上，
    /// 向前跳过它们，避免从 tool group 中间开始压缩区域。
    /// （Hermes: _align_boundary_forward）
    pub(super) fn align_boundary_forward(messages: &[Message], idx: usize) -> usize {
        let mut i = idx;
        while i < messages.len() {
            if matches!(&messages[i], Message::Tool { .. }) {
                i += 1;
            } else {
                break;
            }
        }
        i
    }

    /// 分离尾部（按 token 预算 + soft_ceiling 保护）
    /// 包含 tool group boundary alignment（不切割 tool_call/tool_result 对）
    pub(super) fn split_tail<'a>(&self, messages: &'a [Message]) -> (&'a [Message], &'a [Message]) {
        let target = self.budget.target_tokens();
        let soft_ceiling = self.budget.tail_soft_ceiling();
        let mut used_tokens = 0u64;
        let mut tail_start = messages.len();

        // 从后往前计算，使用 soft_ceiling 防止超大消息中间切割
        for (i, msg) in messages.iter().enumerate().rev() {
            let tokens = estimate_messages_tokens_for_profile(
                std::slice::from_ref(msg),
                self.token_estimate_profile,
            );
            if used_tokens + tokens > soft_ceiling {
                tail_start = i + 1;
                break;
            }
            used_tokens += tokens;
            // 如果在 target 内，继续；超过 target 但在 soft_ceiling 内，也继续
            if used_tokens > target && tail_start == messages.len() {
                // 记录第一个超过 target 的位置，作为备选
                tail_start = i + 1;
            }
        }

        // 确保至少保留一条消息
        if tail_start >= messages.len() && !messages.is_empty() {
            tail_start = messages.len() - 1;
        }

        // ── Tool group boundary alignment ──────────────
        // 如果 tail_start 落在 tool result 中，需要把对应的 assistant 消息也包含进来
        // 如果 tail_start 落在 assistant + tool_calls 中，需要把所有 tool result 也包含进来
        if tail_start < messages.len() {
            // 检查 tail_start 是否在 tool result 链中间
            if let Message::Tool { tool_call_id, .. } = &messages[tail_start] {
                // 找到发起这个 tool_call 的 assistant 消息
                let call_id = tool_call_id.clone();
                for j in (0..tail_start).rev() {
                    if let Message::Assistant {
                        tool_calls: Some(calls),
                        ..
                    } = &messages[j]
                    {
                        if calls.iter().any(|tc| tc.id == call_id) {
                            // 将 tail_start 扩展到 assistant 消息
                            tail_start = j;
                            break;
                        }
                    }
                }
            }

            // 检查 tail_start 是否是带 tool_calls 的 assistant
            if let Message::Assistant {
                tool_calls: Some(calls),
                ..
            } = &messages[tail_start]
            {
                if !calls.is_empty() {
                    // tail 已包含 tail_start 之后的所有消息，
                    // 后续 sanitize 会处理孤立 tool pairs，这里不需要额外边界对齐。
                }
            }
        }

        // 最少保留 3 条消息（Hermes 风格）
        if tail_start >= messages.len().saturating_sub(2) && messages.len() > 3 {
            tail_start = messages.len() - 3;
        }

        (&messages[..tail_start], &messages[tail_start..])
    }

    /// 对中间部分生成摘要（迭代式）
    pub(super) fn summarize_middle(&mut self, messages: &[Message]) -> String {
        if messages.is_empty() {
            return self
                .accumulated_summary
                .as_ref()
                .map(|s| s.to_text())
                .unwrap_or_default();
        }

        // 启发式提取
        let mut new_summary = StructuredSummary::new();
        new_summary.goal = format!("对话包含 {} 条消息", messages.len());

        let mut tool_calls = Vec::new();
        let mut files = Vec::new();
        let mut user_goals = Vec::new();

        for msg in messages {
            match msg {
                Message::User { content } => {
                    // 提取用户目标（第一条用户消息通常是目标描述）
                    if user_goals.is_empty() && content.len() > 10 {
                        user_goals.push(content.chars().take(200).collect::<String>());
                    }
                }
                Message::Assistant {
                    tool_calls: Some(calls),
                    ..
                } => {
                    for tc in calls {
                        if !tool_calls.contains(&tc.name) {
                            tool_calls.push(tc.name.clone());
                        }
                        // 提取文件路径
                        if let Some(path) = tc.arguments.get("path").and_then(|v| v.as_str()) {
                            if !files.contains(&path.to_string()) {
                                files.push(path.to_string());
                            }
                        }
                        // 保留关键命令参数（尤其是编译/测试/诊断命令）
                        if tc.name == "bash" {
                            if let Some(cmd) = tc.arguments["command"]
                                .as_str()
                                .or_else(|| tc.arguments["cmd"].as_str())
                            {
                                let trimmed = cmd.trim();
                                if !trimmed.is_empty() {
                                    let snippet = format!(
                                        "Command: {}",
                                        trimmed.chars().take(180).collect::<String>()
                                    );
                                    if !new_summary.tools_patterns.contains(&snippet) {
                                        new_summary.tools_patterns.push(snippet);
                                    }
                                }
                            }
                        }
                    }
                }
                Message::Tool { content, .. } => {
                    // 只有当工具结果同时包含错误和成功标志时，才认为"错误已解决"
                    let lower = content.to_lowercase();
                    let has_error = lower.contains("error") || lower.contains("failed");
                    let has_success = lower.contains("ok")
                        || lower.contains("success")
                        || lower.contains("passed");
                    if has_error && has_success {
                        new_summary
                            .progress_done
                            .push("遇到错误并已解决".to_string());
                    }
                    // 启发式提取：保留关键工具输出（文件内容、诊断结果等）
                    let trimmed = content.trim();
                    if !trimmed.is_empty()
                        && trimmed.len() > 20
                        && trimmed.len() < 300
                        && (trimmed.contains("API key")
                            || trimmed.contains("secret")
                            || trimmed.contains("password")
                            || trimmed.contains("diagnostic"))
                    {
                        let snippet = trimmed.chars().take(200).collect::<String>();
                        if !new_summary.critical_context.contains(&snippet) {
                            new_summary.critical_context.push(snippet);
                        }
                    }
                    // 长输出中提取失败链路和关键诊断行
                    let lower = content.to_lowercase();
                    if Self::is_critical_tool_output(content)
                        || lower.contains("cargo check")
                        || lower.contains("cargo test")
                    {
                        let important_lines: Vec<String> = content
                            .lines()
                            .map(str::trim)
                            .filter(|l| {
                                let x = l.to_lowercase();
                                !l.is_empty()
                                    && (x.contains("error")
                                        || x.contains("failed")
                                        || x.contains("panic")
                                        || x.contains("warning")
                                        || x.contains("cargo check")
                                        || x.contains("cargo test")
                                        || x.contains("diagnostic")
                                        || x.contains("line "))
                            })
                            .take(6)
                            .map(|s| s.chars().take(180).collect::<String>())
                            .collect();
                        for line in important_lines {
                            if !new_summary.critical_context.contains(&line) {
                                new_summary.critical_context.push(line);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if !user_goals.is_empty() {
            new_summary.goal = user_goals[0].clone();
        }

        if !tool_calls.is_empty() {
            for tool in tool_calls {
                if !new_summary.tools_patterns.contains(&tool) {
                    new_summary.tools_patterns.push(tool);
                }
            }
        }
        if !files.is_empty() {
            new_summary.files_modified = files;
        }

        // 迭代式合并：将新摘要合并到累积摘要
        if let Some(ref mut acc) = self.accumulated_summary {
            acc.merge(&new_summary);
            acc.to_text()
        } else {
            self.accumulated_summary = Some(new_summary.clone());
            new_summary.to_text()
        }
    }

    /// 使用 LLM 生成高质量结构化摘要（异步）
    /// 需要先通过 with_llm_provider() 设置 provider。
    /// Gated by PRIORITY_AGENT_LLM_COMPACTION=1 (default off).
    /// 如果 previous_summary 不为空，做 anchored update 而不是全新摘要。
    pub async fn llm_summarize_middle(
        &self,
        messages: &[Message],
        previous_summary: Option<&str>,
    ) -> Option<String> {
        if !llm_compaction_enabled() {
            return None;
        }
        let provider = self.llm_provider.as_ref()?;
        if messages.is_empty() {
            return None;
        }

        // 构建对话文本
        let mut conversation = String::new();
        for msg in messages {
            let (role, content) = match msg {
                Message::User { content } => ("User", content.as_str()),
                Message::Assistant { content, .. } => ("Assistant", content.as_str()),
                Message::Tool { content, .. } => ("Tool Result", content.as_str()),
                Message::System { content } => ("System", content.as_str()),
            };
            conversation.push_str(&format!("{}: {}\n\n", role, content));
        }

        // LLM compaction contract: strict 8-section template with evidence rules.
        // Summary is continuation context ONLY, not closeout verification proof.
        let mut prompt_parts = Vec::new();

        if let Some(prev) = previous_summary {
            prompt_parts.push(format!(
                "Update the anchored summary below using the new conversation history above.\n\
                 Preserve still-true details, remove stale details, and merge in the new facts.\n\n\
                 <previous-summary>\n{}\n</previous-summary>",
                prev
            ));
        } else {
            prompt_parts.push(
                "Create a new anchored summary from the conversation history above.".to_string(),
            );
        }

        prompt_parts.push(
            "Output exactly these sections and keep the order:\n\n\
             ## Goal\n\
             ## Constraints\n\
             ## Progress\n\
             ## Key Decisions\n\
             ## Relevant Files\n\
             ## Next Steps\n\
             ## Critical Context\n\
             ## Tools & Patterns\n\n\
             Rules:\n\
             - Keep every section, even when empty — write \"(none)\" for empty ones.\n\
             - Use terse bullets, not prose paragraphs.\n\
             - Preserve exact file paths, commands, error strings, and identifiers.\n\
             - Mark validation evidence as historical unless raw output is preserved.\n\
             - Do not claim tests passed unless raw test output evidence remains.\n\
             - Do not omit unresolved blockers.\n\
             - Do not include secrets or API keys.\n\
             - This summary is continuation context, NOT verification proof."
                .to_string(),
        );

        prompt_parts.push(format!(
            "Conversation to summarize:\n\n{}",
            &conversation.chars().take(16000).collect::<String>()
        ));

        let prompt = prompt_parts.join("\n\n");
        let mut summary_messages = Vec::new();
        if let Some(prefix) = self.llm_summary_stable_prefix.as_deref() {
            summary_messages.push(crate::services::api::Message::system(prefix));
        }
        summary_messages.push(crate::services::api::Message::user(&prompt));

        let request = crate::services::api::ChatRequest::new(&self.llm_model)
            .with_messages(summary_messages)
            .with_output_cap(Some(2048));

        match provider.chat(request).await {
            Ok(response) => {
                debug!("LLM summary generated ({} chars)", response.content.len());
                Some(response.content)
            }
            Err(e) => {
                warn!("LLM summarization failed: {}, falling back to heuristic", e);
                None
            }
        }
    }

    /// 检查是否有 LLM provider 可用
    pub fn has_llm_provider(&self) -> bool {
        self.llm_provider.is_some()
    }

    /// 是否在压缩摘要中保留技能提醒 marker。
    /// 当前默认 true：skills 在会话启动时加载，压缩中始终保留。
    pub fn preserve_skills_marker(&self) -> bool {
        self.preserve_skills_marker
    }

    /// 标记技能应被保留。当前默认即为 true，仅用于确保 marker 不被意外关闭。
    pub fn mark_skills_preserved(&mut self) {
        self.preserve_skills_marker = true;
    }

    /// 记录压缩失败（进入冷却期）
    pub fn record_failure(&mut self) {
        self.last_failure_time = Some(std::time::Instant::now());
        debug!(
            "Compression failed, entering cooldown for {}s",
            self.cooldown_secs
        );
    }

    /// 校验工具调用对的完整性
    /// 确保每个 tool_call 都有对应的 tool result，反之亦然
    pub(super) fn sanitize_tool_pairs(mut messages: Vec<Message>) -> Vec<Message> {
        let mut pending_tool_calls: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut to_remove = Vec::new();

        for (i, msg) in messages.iter().enumerate() {
            match msg {
                Message::Assistant {
                    tool_calls: Some(calls),
                    ..
                } => {
                    for tc in calls {
                        pending_tool_calls.insert(tc.id.clone(), i);
                    }
                }
                Message::Tool { tool_call_id, .. } => {
                    if pending_tool_calls.remove(tool_call_id).is_none() {
                        // 没有对应的 tool_call，标记移除
                        to_remove.push(i);
                    }
                }
                _ => {}
            }
        }

        // 移除孤立的 tool result
        for i in to_remove.into_iter().rev() {
            messages.remove(i);
        }

        // 为没有 result 的 tool_call 插入 stub
        if !pending_tool_calls.is_empty() {
            debug!(
                "Found {} orphaned tool calls, inserting stubs",
                pending_tool_calls.len()
            );
            for (tc_id, assistant_idx) in &pending_tool_calls {
                // 在 assistant 消息之后插入 stub tool result
                let insert_pos = assistant_idx + 1;
                if insert_pos <= messages.len() {
                    messages.insert(
                        insert_pos,
                        Message::Tool {
                            tool_call_id: tc_id.clone(),
                            content: "[Tool result lost during context compression]".to_string(),
                        },
                    );
                }
            }
        }

        messages
    }

    /// 获取当前累积摘要的引用
    pub fn accumulated_summary(&self) -> Option<&StructuredSummary> {
        self.accumulated_summary.as_ref()
    }

    /// 获取压缩元数据历史
    pub fn compact_metadata_history(&self) -> &[CompactMetadata] {
        &self.compact_metadata_history
    }

    /// 获取最近一次 compact boundary 元数据
    pub fn latest_compact_metadata(&self) -> Option<&CompactMetadata> {
        self.compact_metadata_history.last()
    }

    fn record_compaction(&mut self, mut record: CompactionRuntimeRecord) {
        record.normalize_provenance();
        self.compaction_records.push(record);
    }

    /// 获取运行时压缩记录（策略、来源和 compact boundary）。
    pub fn compaction_records(&self) -> &[CompactionRuntimeRecord] {
        &self.compaction_records
    }

    pub fn compaction_attempt_records(&self) -> &[CompactionAttemptRecord] {
        &self.compaction_attempt_records
    }

    pub fn compaction_circuit_open(&self) -> bool {
        self.consecutive_compaction_failures >= self.max_consecutive_compaction_failures
            || self.consecutive_no_gain_compactions >= self.max_consecutive_no_gain_compactions
    }

    pub fn record_compaction_decision(
        &mut self,
        input: CompactionAttemptInput,
    ) -> CompactionAttemptRecord {
        match input.decision {
            CompactionDecision::Compacted | CompactionDecision::Recovered => {
                self.consecutive_compaction_failures = 0;
                self.consecutive_no_gain_compactions = 0;
            }
            CompactionDecision::NoGain => {
                self.consecutive_no_gain_compactions =
                    self.consecutive_no_gain_compactions.saturating_add(1);
            }
            CompactionDecision::Failed => {
                self.consecutive_compaction_failures =
                    self.consecutive_compaction_failures.saturating_add(1);
            }
            _ => {}
        }
        let record = CompactionAttemptRecord {
            trigger: input.trigger,
            strategy: input.strategy,
            decision: input.decision,
            before_tokens: input.before_tokens,
            after_tokens: input.after_tokens,
            messages_before: input.messages_before,
            messages_after: input.messages_after,
            reason: input.reason,
            attempt_index: self
                .compaction_attempt_records
                .len()
                .saturating_add(1)
                .try_into()
                .unwrap_or(u32::MAX),
            consecutive_no_gain: self.consecutive_no_gain_compactions,
            consecutive_failures: self.consecutive_compaction_failures,
            circuit_open: self.compaction_circuit_open(),
            boundary_id: input.boundary_id,
        };
        self.compaction_attempt_records.push(record.clone());
        record
    }

    pub fn annotate_compaction_record_trigger(&mut self, index: usize, trigger: impl Into<String>) {
        if let Some(record) = self.compaction_records.get_mut(index) {
            record.trigger = Some(trigger.into());
            record.normalize_provenance();
        }
    }

    /// 获取最近一次运行时压缩记录。
    pub fn latest_compaction_record(&self) -> Option<&CompactionRuntimeRecord> {
        self.compaction_records.last()
    }

    /// 获取压缩统计
    pub fn stats(&self) -> CompressionStats {
        let savings_rate = if self.total_tokens_before > 0 {
            self.total_tokens_before
                .saturating_sub(self.total_tokens_after)
                .saturating_mul(100)
                / self.total_tokens_before
        } else {
            0
        };
        CompressionStats {
            compression_count: self.compression_count,
            max_context_tokens: self.budget.max_context_tokens,
            available_tokens: self.budget.available_for_history(),
            has_accumulated_summary: self.accumulated_summary.is_some(),
            in_cooldown: self.is_in_cooldown(),
            total_tokens_before: self.total_tokens_before,
            total_tokens_after: self.total_tokens_after,
            llm_compression_attempts: self.llm_compression_attempts,
            llm_compression_failures: self.llm_compression_failures,
            savings_rate,
            session_duration_secs: self.session_start.elapsed().as_secs(),
            message_count: 0, // caller should fill this
            time_based_enabled: self.time_config.enabled,
            token_counter: self.token_counter_label(),
        }
    }
}

fn llm_compaction_enabled() -> bool {
    matches!(
        std::env::var("PRIORITY_AGENT_LLM_COMPACTION")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

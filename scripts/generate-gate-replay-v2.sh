#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OUT="docs/workflow/gate-replay-samples-v2.json"
mkdir -p "$(dirname "$OUT")"

echo "[" > "$OUT"
first=1
append() {
  local task="$1"
  local complexity="$2"
  if [[ $first -eq 0 ]]; then
    echo "," >> "$OUT"
  fi
  first=0
  printf '  {"task_description":"%s", "complexity":"%s"}' "${task//\"/\\\"}" "$complexity" >> "$OUT"
}

for i in $(seq 1 80); do
  case $((i % 8)) in
    0) append "修复第${i}处文案 typo" "simple" ;;
    1) append "查看模块${i}的当前配置" "simple" ;;
    2) append "列出 src 下第${i}组文件" "simple" ;;
    3) append "修改第${i}个注释拼写" "simple" ;;
    4) append "更新第${i}处版本号展示" "simple" ;;
    5) append "find 第${i}个 TODO 位置" "simple" ;;
    6) append "grep 第${i}个错误关键字" "simple" ;;
    7) append "调整第${i}个开关默认值" "simple" ;;
  esac
done

for i in $(seq 1 60); do
  case $((i % 6)) in
    0) append "分析认证链路第${i}阶段并给出优化计划" "medium" ;;
    1) append "评估 workflow 与 gate 在场景${i}的协同策略" "medium" ;;
    2) append "设计跨模块发布流程第${i}版并定义约束" "medium" ;;
    3) append "梳理成本追踪在会话${i}中的改进方向" "medium" ;;
    4) append "规划第${i}轮稳定性改造的阶段目标" "medium" ;;
    5) append "制定场景${i}的回退与风控策略" "medium" ;;
  esac
done

for i in $(seq 1 60); do
  case $((i % 6)) in
    0) append "重构第${i}批核心模块并解耦架构" "complex" ;;
    1) append "迁移第${i}组数据库 schema 到新版本" "complex" ;;
    2) append "新增第${i}个完整子系统并接入现有架构" "complex" ;;
    3) append "执行第${i}轮跨模块重构与全局替换" "complex" ;;
    4) append "替换第${i}套底层引擎并升级整体框架" "complex" ;;
    5) append "批量删除第${i}组旧实现并迁移依赖" "complex" ;;
  esac
done

echo >> "$OUT"
echo "]" >> "$OUT"

echo "generated $OUT with $(rg -n '"tool"' "$OUT" 2>/dev/null | wc -l | tr -d ' ') entries" >/dev/null 2>&1 || true
count=$(rg -n '"complexity"' "$OUT" | wc -l | tr -d ' ')
echo "Generated $OUT ($count samples)"

//! 插件签名信任基础设施
//!
//! 提供 Ed25519 签名验证与信任模式管理

use super::{PluginManifest, PluginValidationIssue};

/// 信任模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustMode {
    /// 要求有效 Ed25519 签名
    Strict,
    /// 缺失/无效签名时仅警告，但仍允许
    Warn,
    /// 忽略签名
    Off,
}

impl TrustMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "strict" => TrustMode::Strict,
            "off" | "none" | "disabled" => TrustMode::Off,
            _ => TrustMode::Warn,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            TrustMode::Strict => "strict",
            TrustMode::Warn => "warn",
            TrustMode::Off => "off",
        }
    }
}

/// 签名验证器
pub struct SignatureVerifier;

impl SignatureVerifier {
    /// 验证插件清单签名
    /// 返回 Ok(true) = 签名有效，Ok(false) = 签名缺失或无效，Err = 验证过程出错
    pub fn verify_manifest(manifest: &PluginManifest) -> Result<bool, String> {
        let Some(ref pub_b64) = manifest.public_key else {
            return Ok(false);
        };
        let Some(ref sig_b64) = manifest.signature else {
            return Ok(false);
        };

        use base64::Engine;
        let pub_bytes = base64::engine::general_purpose::STANDARD
            .decode(pub_b64)
            .map_err(|e| format!("Failed to decode public_key: {}", e))?;
        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(sig_b64)
            .map_err(|e| format!("Failed to decode signature: {}", e))?;

        if pub_bytes.len() != 32 {
            return Err(format!(
                "Invalid public key length: expected 32, got {}",
                pub_bytes.len()
            ));
        }
        if sig_bytes.len() != 64 {
            return Err(format!(
                "Invalid signature length: expected 64, got {}",
                sig_bytes.len()
            ));
        }

        let mut pub_arr = [0u8; 32];
        pub_arr.copy_from_slice(&pub_bytes);
        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&sig_bytes);

        let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&pub_arr)
            .map_err(|e| format!("Invalid Ed25519 public key: {}", e))?;
        let signature = ed25519_dalek::Signature::from_bytes(&sig_arr);

        // 构建待验证的 canonical 内容（排除 signature 字段本身）
        let canonical = canonical_manifest_toml(manifest)?;

        match verifying_key.verify_strict(canonical.as_bytes(), &signature) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

/// 生成用于签名验证的 canonical TOML（排除 signature 字段）
fn canonical_manifest_toml(manifest: &PluginManifest) -> Result<String, String> {
    let mut stripped = manifest.clone();
    stripped.signature = None;
    toml::to_string(&stripped).map_err(|e| {
        format!(
            "Failed to serialize manifest for signature verification: {}",
            e
        )
    })
}

/// 使用私钥为插件清单生成签名
///
/// 私钥为 base64 编码的 32 字节 Ed25519 secret key。
/// 返回 (public_key_base64, signature_base64)。
pub fn sign_manifest(
    manifest: &PluginManifest,
    private_key_b64: &str,
) -> Result<(String, String), String> {
    use base64::Engine;
    use ed25519_dalek::Signer;

    let secret_bytes = base64::engine::general_purpose::STANDARD
        .decode(private_key_b64)
        .map_err(|e| format!("Failed to decode private_key: {}", e))?;

    if secret_bytes.len() != 32 {
        return Err(format!(
            "Invalid private key length: expected 32, got {}",
            secret_bytes.len()
        ));
    }

    let mut secret_arr = [0u8; 32];
    secret_arr.copy_from_slice(&secret_bytes);

    let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret_arr);
    let verifying_key = signing_key.verifying_key();

    let public_key_b64 = base64::engine::general_purpose::STANDARD.encode(verifying_key.to_bytes());

    // 先设置 public_key（canonical 内容包含 public_key 但不包含 signature）
    let mut manifest_with_pk = manifest.clone();
    manifest_with_pk.public_key = Some(public_key_b64.clone());
    manifest_with_pk.signature = None;

    let canonical = canonical_manifest_toml(&manifest_with_pk)?;
    let signature = signing_key.sign(canonical.as_bytes());
    let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

    Ok((public_key_b64, signature_b64))
}

/// 生成新的 Ed25519 密钥对
/// 返回 (private_key_base64, public_key_base64)
pub fn generate_keypair() -> (String, String) {
    use base64::Engine;
    use rand::RngCore;

    let mut secret = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut secret);
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();

    let private_b64 = base64::engine::general_purpose::STANDARD.encode(secret);
    let public_b64 = base64::engine::general_purpose::STANDARD.encode(verifying_key.to_bytes());

    (private_b64, public_b64)
}

/// 校验插件签名并生成验证问题列表
pub fn validate_signature(
    manifest: &PluginManifest,
    mode: TrustMode,
) -> Vec<PluginValidationIssue> {
    let mut issues = Vec::new();

    if mode == TrustMode::Off {
        return issues;
    }

    let has_sig = manifest.signature.is_some();
    let has_pk = manifest.public_key.is_some();

    if !has_sig && !has_pk {
        let msg = "Plugin manifest is not signed (missing signature and public_key)".to_string();
        if mode == TrustMode::Strict {
            issues.push(PluginValidationIssue {
                severity: "error".to_string(),
                message: msg,
            });
        } else {
            issues.push(PluginValidationIssue {
                severity: "warning".to_string(),
                message: msg,
            });
        }
        return issues;
    }

    if has_sig != has_pk {
        issues.push(PluginValidationIssue {
            severity: "error".to_string(),
            message: "Plugin manifest has signature/public_key mismatch (both required)"
                .to_string(),
        });
        return issues;
    }

    match SignatureVerifier::verify_manifest(manifest) {
        Ok(true) => {
            issues.push(PluginValidationIssue {
                severity: "info".to_string(),
                message: "Signature is valid".to_string(),
            });
        }
        Ok(false) => {
            issues.push(PluginValidationIssue {
                severity: "error".to_string(),
                message: "Signature verification failed (invalid signature or tampered manifest)"
                    .to_string(),
            });
        }
        Err(e) => {
            issues.push(PluginValidationIssue {
                severity: "error".to_string(),
                message: format!("Signature verification error: {}", e),
            });
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn test_manifest() -> PluginManifest {
        PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            enabled: true,
            entry_command: Some("sh".to_string()),
            entry_args: vec![],
            tool_name: None,
            tool_description: None,
            tool_timeout_secs: None,
            signature: None,
            public_key: None,
        }
    }

    #[test]
    fn test_trust_mode_parsing() {
        assert_eq!(TrustMode::from_str("strict"), TrustMode::Strict);
        assert_eq!(TrustMode::from_str("STRICT"), TrustMode::Strict);
        assert_eq!(TrustMode::from_str("warn"), TrustMode::Warn);
        assert_eq!(TrustMode::from_str("off"), TrustMode::Off);
        assert_eq!(TrustMode::from_str("none"), TrustMode::Off);
        assert_eq!(TrustMode::from_str("unknown"), TrustMode::Warn);
    }

    #[test]
    fn test_validate_signature_off_mode_ignores_all() {
        let manifest = test_manifest();
        let issues = validate_signature(&manifest, TrustMode::Off);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_validate_signature_missing_fields_warn() {
        let manifest = test_manifest();
        let issues = validate_signature(&manifest, TrustMode::Warn);
        assert!(issues.iter().any(|i| i.severity == "warning"));
    }

    #[test]
    fn test_validate_signature_missing_fields_strict() {
        let manifest = test_manifest();
        let issues = validate_signature(&manifest, TrustMode::Strict);
        assert!(issues.iter().any(|i| i.severity == "error"));
    }

    #[test]
    fn test_validate_signature_mismatch() {
        let mut manifest = test_manifest();
        manifest.signature = Some("abc".to_string());
        manifest.public_key = None;
        let issues = validate_signature(&manifest, TrustMode::Warn);
        assert!(issues.iter().any(|i| i.message.contains("mismatch")));
    }

    #[test]
    fn test_signature_roundtrip_valid() {
        use base64::Engine;
        use rand::RngCore;
        let mut secret = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut secret);
        let signing_key = SigningKey::from_bytes(&secret);
        let verifying_key = signing_key.verifying_key();

        let mut manifest = test_manifest();
        // 先设置 public_key，再计算 canonical 内容并签名
        manifest.public_key =
            Some(base64::engine::general_purpose::STANDARD.encode(verifying_key.to_bytes()));
        let canonical = canonical_manifest_toml(&manifest).unwrap();
        let signature = signing_key.sign(canonical.as_bytes());
        manifest.signature =
            Some(base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()));

        let result = SignatureVerifier::verify_manifest(&manifest);
        assert_eq!(result, Ok(true));

        let issues = validate_signature(&manifest, TrustMode::Strict);
        assert!(issues
            .iter()
            .any(|i| i.severity == "info" && i.message.contains("valid")));
    }

    #[test]
    fn test_signature_tampered_manifest_fails() {
        use base64::Engine;
        use rand::RngCore;
        let mut secret = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut secret);
        let signing_key = SigningKey::from_bytes(&secret);
        let verifying_key = signing_key.verifying_key();

        let mut manifest = test_manifest();
        manifest.public_key =
            Some(base64::engine::general_purpose::STANDARD.encode(verifying_key.to_bytes()));
        let canonical = canonical_manifest_toml(&manifest).unwrap();
        let signature = signing_key.sign(canonical.as_bytes());
        manifest.signature =
            Some(base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()));

        // 篡改清单
        manifest.description = "tampered".to_string();
        let result = SignatureVerifier::verify_manifest(&manifest);
        assert_eq!(result, Ok(false));
    }
}

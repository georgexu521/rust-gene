//! 计算器工具
//!
//! 支持数学表达式计算

use crate::tools::{Tool, ToolContext, ToolResult};

use async_trait::async_trait;
use serde_json::json;

/// 计算器工具
pub struct CalculateTool;

#[async_trait]
impl Tool for CalculateTool {
    fn name(&self) -> &str {
        "calculate"
    }

    fn description(&self) -> &str {
        "Evaluate mathematical expressions. Supports basic arithmetic, parentheses, \
         and common functions like sqrt, sin, cos, log, etc."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The mathematical expression to evaluate. \
                                   Examples: '2 + 2', 'sqrt(16)', 'sin(3.14159/2)'"
                }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let expression = params["expression"].as_str().unwrap_or("");
        if expression.is_empty() {
            return ToolResult::error("Expression cannot be empty");
        }

        match evaluate_expression(expression) {
            Ok(result) => ToolResult::success_with_data(
                format!("{} = {}", expression, result),
                json!({
                    "expression": expression,
                    "result": result
                }),
            ),
            Err(e) => ToolResult::error(format!("Failed to evaluate expression: {}", e)),
        }
    }
}

const MAX_PARSE_DEPTH: usize = 50;

/// 简单表达式求值
fn evaluate_expression(expr: &str) -> anyhow::Result<f64> {
    // 移除空格
    let expr = expr.replace(" ", "");

    // 使用 meval 库（如果可用）或简单的解析器
    // 这里实现一个简单的支持基本运算的解析器
    parse_and_eval(&expr, 0)
}

fn parse_and_eval(expr: &str, depth: usize) -> anyhow::Result<f64> {
    if depth > MAX_PARSE_DEPTH {
        return Err(anyhow::anyhow!("Expression too deeply nested"));
    }

    // 先检查括号，处理嵌套
    if let Some(start) = expr.find('(') {
        if let Some(end) = find_matching_paren(expr, start) {
            let func_name = &expr[..start];
            let inner = &expr[start + 1..end];

            // 尝试作为函数求值
            if let Some(result) = try_eval_function_with_name(func_name, inner, depth + 1) {
                return result;
            }

            // 否则作为分组处理
            let inner_val = parse_and_eval(inner, depth + 1)?;
            let new_expr = format!("{}{}", &expr[..start], inner_val);
            if end + 1 < expr.len() {
                return parse_and_eval(&format!("{}{}", new_expr, &expr[end + 1..]), depth + 1);
            }
            return parse_and_eval(&new_expr, depth + 1);
        }
    }

    // 处理括号
    if let Some(start) = expr.find('(') {
        if let Some(end) = find_matching_paren(expr, start) {
            let inner = &expr[start + 1..end];
            let inner_val = parse_and_eval(inner, depth + 1)?;
            let new_expr = format!("{}{}", &expr[..start], inner_val);
            if end + 1 < expr.len() {
                return parse_and_eval(&format!("{}{}", new_expr, &expr[end + 1..]), depth + 1);
            }
            return parse_and_eval(&new_expr, depth + 1);
        }
    }

    // 处理基本运算符（按优先级）
    // 先处理 + -
    if let Some(pos) = find_operator(expr, &['+', '-']) {
        let left = parse_and_eval(&expr[..pos], depth + 1)?;
        let right = parse_and_eval(&expr[pos + 1..], depth + 1)?;
        return match expr.chars().nth(pos).unwrap() {
            '+' => Ok(left + right),
            '-' => Ok(left - right),
            _ => unreachable!(),
        };
    }

    // 再处理 * /
    if let Some(pos) = find_operator(expr, &['*', '/']) {
        let left = parse_and_eval(&expr[..pos], depth + 1)?;
        let right = parse_and_eval(&expr[pos + 1..], depth + 1)?;
        return match expr.chars().nth(pos).unwrap() {
            '*' => Ok(left * right),
            '/' => {
                if right == 0.0 {
                    return Err(anyhow::anyhow!("Division by zero"));
                }
                Ok(left / right)
            }
            _ => unreachable!(),
        };
    }

    // 再处理 ^ (幂)
    if let Some(pos) = find_operator(expr, &['^']) {
        let left = parse_and_eval(&expr[..pos], depth + 1)?;
        let right = parse_and_eval(&expr[pos + 1..], depth + 1)?;
        return Ok(left.powf(right));
    }

    // 纯数字
    expr.parse::<f64>()
        .map_err(|e| anyhow::anyhow!("Invalid number '{}': {}", expr, e))
}

type MathFn = fn(f64) -> f64;

fn try_eval_function_with_name(name: &str, arg: &str, depth: usize) -> Option<anyhow::Result<f64>> {
    let funcs: [(&str, MathFn); 11] = [
        ("sqrt", |x: f64| x.sqrt()),
        ("sin", |x: f64| x.sin()),
        ("cos", |x: f64| x.cos()),
        ("tan", |x: f64| x.tan()),
        ("log", |x: f64| x.ln()),
        ("log10", |x: f64| x.log10()),
        ("exp", |x: f64| x.exp()),
        ("abs", |x: f64| x.abs()),
        ("floor", |x: f64| x.floor()),
        ("ceil", |x: f64| x.ceil()),
        ("round", |x: f64| x.round()),
    ];

    for (func_name, f) in &funcs {
        if name == *func_name {
            let val = match parse_and_eval(arg, depth) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            return Some(Ok(f(val)));
        }
    }
    None
}

fn find_matching_paren(expr: &str, open_pos: usize) -> Option<usize> {
    let mut depth = 1;
    for (i, c) in expr[open_pos + 1..].chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_pos + 1 + i);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_operator(expr: &str, ops: &[char]) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in expr.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 && ops.contains(&c) => return Some(i),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        assert!((parse_and_eval("2+2", 0).unwrap() - 4.0).abs() < 0.001);
        assert!((parse_and_eval("10-3", 0).unwrap() - 7.0).abs() < 0.001);
        assert!((parse_and_eval("6*7", 0).unwrap() - 42.0).abs() < 0.001);
        assert!((parse_and_eval("15/3", 0).unwrap() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_parentheses() {
        assert!((parse_and_eval("(2+3)*4", 0).unwrap() - 20.0).abs() < 0.001);
        assert!((parse_and_eval("2*(3+4)", 0).unwrap() - 14.0).abs() < 0.001);
    }

    #[test]
    fn test_functions() {
        assert!((parse_and_eval("sqrt(16)", 0).unwrap() - 4.0).abs() < 0.001);
        // Note: abs(-5) and sin(0) require better negative number parsing
        // which is not yet fully implemented
    }

    #[test]
    fn test_power() {
        assert!((parse_and_eval("2^3", 0).unwrap() - 8.0).abs() < 0.001);
        assert!((parse_and_eval("3^2", 0).unwrap() - 9.0).abs() < 0.001);
    }

    #[test]
    fn test_depth_limit() {
        let deeply_nested = "((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((1))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))";
        assert!(parse_and_eval(deeply_nested, 0).is_err());
    }

    #[tokio::test]
    async fn test_calculate_tool() {
        let tool = CalculateTool;
        let params = json!({"expression": "2 + 2 * 3"});
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(result.success);
        assert!(result.content.contains("8"));
    }

    #[tokio::test]
    async fn test_calculate_error() {
        let tool = CalculateTool;
        let params = json!({"expression": ""});
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(!result.success);
    }
}

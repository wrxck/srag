// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use srag_common::types::Language;

#[test]
fn test_language_from_extension_rust() {
    assert_eq!(Language::from_extension("rs"), Language::Rust);
}

#[test]
fn test_language_from_extension_python() {
    assert_eq!(Language::from_extension("py"), Language::Python);
    assert_eq!(Language::from_extension("pyi"), Language::Python);
}

#[test]
fn test_language_from_extension_javascript() {
    assert_eq!(Language::from_extension("js"), Language::JavaScript);
    assert_eq!(Language::from_extension("mjs"), Language::JavaScript);
    assert_eq!(Language::from_extension("cjs"), Language::JavaScript);
}

#[test]
fn test_language_from_extension_typescript() {
    assert_eq!(Language::from_extension("ts"), Language::TypeScript);
    assert_eq!(Language::from_extension("mts"), Language::TypeScript);
    assert_eq!(Language::from_extension("cts"), Language::TypeScript);
    assert_eq!(Language::from_extension("tsx"), Language::TypeScript);
    assert_eq!(Language::from_extension("jsx"), Language::TypeScript);
}

#[test]
fn test_language_from_extension_go() {
    assert_eq!(Language::from_extension("go"), Language::Go);
}

#[test]
fn test_language_from_extension_c() {
    assert_eq!(Language::from_extension("c"), Language::C);
    assert_eq!(Language::from_extension("h"), Language::C);
}

#[test]
fn test_language_from_extension_cpp() {
    assert_eq!(Language::from_extension("cpp"), Language::Cpp);
    assert_eq!(Language::from_extension("cc"), Language::Cpp);
    assert_eq!(Language::from_extension("cxx"), Language::Cpp);
    assert_eq!(Language::from_extension("hpp"), Language::Cpp);
    assert_eq!(Language::from_extension("hxx"), Language::Cpp);
    assert_eq!(Language::from_extension("hh"), Language::Cpp);
}

#[test]
fn test_language_from_extension_java() {
    assert_eq!(Language::from_extension("java"), Language::Java);
}

#[test]
fn test_language_from_extension_ruby() {
    assert_eq!(Language::from_extension("rb"), Language::Ruby);
}

#[test]
fn test_language_from_extension_shell() {
    assert_eq!(Language::from_extension("sh"), Language::Shell);
    assert_eq!(Language::from_extension("bash"), Language::Shell);
    assert_eq!(Language::from_extension("zsh"), Language::Shell);
    assert_eq!(Language::from_extension("fish"), Language::Shell);
}

#[test]
fn test_language_from_extension_markdown() {
    assert_eq!(Language::from_extension("md"), Language::Markdown);
    assert_eq!(Language::from_extension("mdx"), Language::Markdown);
}

#[test]
fn test_language_from_extension_config() {
    assert_eq!(Language::from_extension("toml"), Language::Toml);
    assert_eq!(Language::from_extension("yml"), Language::Yaml);
    assert_eq!(Language::from_extension("yaml"), Language::Yaml);
    assert_eq!(Language::from_extension("json"), Language::Json);
}

#[test]
fn test_language_from_extension_web() {
    assert_eq!(Language::from_extension("html"), Language::Html);
    assert_eq!(Language::from_extension("htm"), Language::Html);
    assert_eq!(Language::from_extension("css"), Language::Css);
    assert_eq!(Language::from_extension("scss"), Language::Css);
    assert_eq!(Language::from_extension("less"), Language::Css);
}

#[test]
fn test_language_from_extension_sql() {
    assert_eq!(Language::from_extension("sql"), Language::Sql);
}

#[test]
fn test_language_from_extension_unknown() {
    assert_eq!(Language::from_extension("xyz"), Language::Unknown);
    assert_eq!(Language::from_extension(""), Language::Unknown);
    assert_eq!(Language::from_extension("RS"), Language::Unknown);
}

#[test]
fn test_language_from_filename_env() {
    assert_eq!(Language::from_filename(".env"), Some(Language::Env));
    assert_eq!(Language::from_filename(".ENV"), Some(Language::Env));
    assert_eq!(Language::from_filename(".env.local"), Some(Language::Env));
    assert_eq!(
        Language::from_filename(".env.production"),
        Some(Language::Env)
    );
    assert_eq!(
        Language::from_filename("development.env"),
        Some(Language::Env)
    );
}

#[test]
fn test_language_from_filename_non_env() {
    assert_eq!(Language::from_filename("main.rs"), None);
    assert_eq!(Language::from_filename("environment"), None);
    assert_eq!(Language::from_filename("envfile"), None);
}

#[test]
fn test_language_as_str_round_trip() {
    let languages = [
        Language::Rust,
        Language::Python,
        Language::JavaScript,
        Language::TypeScript,
        Language::Go,
        Language::C,
        Language::Cpp,
        Language::Java,
        Language::Ruby,
        Language::Shell,
        Language::Markdown,
        Language::Toml,
        Language::Yaml,
        Language::Json,
        Language::Html,
        Language::Css,
        Language::Sql,
        Language::Env,
        Language::Unknown,
    ];
    for lang in languages {
        let s = lang.as_str();
        assert!(!s.is_empty());
        let json_str = serde_json::to_string(&lang).unwrap();
        let parsed: Language = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed, lang);
    }
}

#[test]
fn test_language_has_tree_sitter_support() {
    assert!(Language::Rust.has_tree_sitter_support());
    assert!(Language::Python.has_tree_sitter_support());
    assert!(Language::JavaScript.has_tree_sitter_support());
    assert!(Language::TypeScript.has_tree_sitter_support());
    assert!(Language::Go.has_tree_sitter_support());
    assert!(Language::C.has_tree_sitter_support());
    assert!(Language::Cpp.has_tree_sitter_support());
    assert!(Language::Java.has_tree_sitter_support());
    assert!(Language::Ruby.has_tree_sitter_support());

    assert!(!Language::Shell.has_tree_sitter_support());
    assert!(!Language::Markdown.has_tree_sitter_support());
    assert!(!Language::Toml.has_tree_sitter_support());
    assert!(!Language::Yaml.has_tree_sitter_support());
    assert!(!Language::Json.has_tree_sitter_support());
    assert!(!Language::Html.has_tree_sitter_support());
    assert!(!Language::Css.has_tree_sitter_support());
    assert!(!Language::Sql.has_tree_sitter_support());
    assert!(!Language::Env.has_tree_sitter_support());
    assert!(!Language::Unknown.has_tree_sitter_support());
}

#[test]
fn test_language_serde_lowercase() {
    assert_eq!(serde_json::to_string(&Language::Rust).unwrap(), "\"rust\"");
    assert_eq!(
        serde_json::to_string(&Language::JavaScript).unwrap(),
        "\"javascript\""
    );
    assert_eq!(
        serde_json::to_string(&Language::TypeScript).unwrap(),
        "\"typescript\""
    );
    assert_eq!(serde_json::to_string(&Language::Cpp).unwrap(), "\"cpp\"");
}

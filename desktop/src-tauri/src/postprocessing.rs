//! Text post-processing utilities for transcription cleanup

use crate::settings::{ReplacementRule, Settings};
use regex::Regex;

/// Capitalizes the first letter of each sentence.
/// A sentence is detected after: `.`, `!`, `?`, or at the start of the string.
pub fn capitalize_sentences(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut result = String::with_capacity(text.len());
    let mut capitalize_next = true;

    for ch in text.chars() {
        if capitalize_next && ch.is_alphabetic() {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
            if ch == '.' || ch == '!' || ch == '?' {
                capitalize_next = true;
            }
        }
    }

    result
}

/// Removes filler words from the text.
/// Handles multi-word fillers like "you know" and preserves sentence structure.
pub fn remove_filler_words(text: &str, filler_words: &[String]) -> String {
    if filler_words.is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();

    // Sort filler words by length (longest first) to avoid partial matches
    let mut sorted_fillers: Vec<&String> = filler_words.iter().collect();
    sorted_fillers.sort_by(|a, b| b.len().cmp(&a.len()));

    for filler in sorted_fillers {
        if filler.is_empty() {
            continue;
        }

        // Create a case-insensitive regex that matches the filler word
        // with word boundaries to avoid matching parts of other words
        let pattern = format!(r"(?i)\b{}\b\s*", regex::escape(filler));
        if let Ok(re) = Regex::new(&pattern) {
            result = re.replace_all(&result, " ").to_string();
        }
    }

    // Clean up extra whitespace
    let whitespace_re = Regex::new(r"\s+").unwrap();
    result = whitespace_re.replace_all(&result, " ").to_string();
    result.trim().to_string()
}

/// Applies custom find/replace rules to the text.
pub fn apply_replacements(text: &str, rules: &[ReplacementRule]) -> String {
    let mut result = text.to_string();

    for rule in rules {
        if rule.find.is_empty() {
            continue;
        }

        // Case-insensitive replacement
        let pattern = format!(r"(?i){}", regex::escape(&rule.find));
        if let Ok(re) = Regex::new(&pattern) {
            result = re.replace_all(&result, rule.replace.as_str()).to_string();
        }
    }

    result
}

/// Removes all punctuation from the text.
pub fn remove_punctuation(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    for ch in text.chars() {
        if !ch.is_ascii_punctuation() {
            result.push(ch);
        }
    }

    // Clean up extra whitespace that may result from removing punctuation
    let whitespace_re = Regex::new(r"\s+").unwrap();
    whitespace_re.replace_all(&result, " ").trim().to_string()
}

/// Normalizes a word for comparison by lowercasing and removing punctuation.
fn normalize_word(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

/// Collapses consecutive duplicate words (e.g., "the the" → "the").
fn collapse_duplicate_words(text: &str) -> String {
    let mut output: Vec<&str> = Vec::new();
    let mut prev_norm: Option<String> = None;

    for word in text.split_whitespace() {
        let norm = normalize_word(word);
        if let Some(prev) = &prev_norm {
            if *prev == norm {
                continue;
            }
        }
        output.push(word);
        prev_norm = Some(norm);
    }

    output.join(" ")
}

/// Collapses repeated multi-word phrases (e.g., "I need you, I need you" → "I need you").
/// Handles phrases up to 8 words long.
fn collapse_repeated_phrases(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < 4 {
        return text.to_string();
    }

    let max_phrase_len = 8; // Maximum phrase length to detect
    let mut output: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < words.len() {
        let remaining = words.len() - i;
        let max_n = (remaining / 2).min(max_phrase_len);
        let mut collapsed = false;

        // Try to find repeated phrases, starting with longer ones
        for n in (2..=max_n).rev() {
            // Count how many times this phrase repeats consecutively
            let mut repeat_count = 1;
            let mut j = i + n;

            while j + n <= words.len() {
                let mut matches = true;
                for k in 0..n {
                    if normalize_word(words[i + k]) != normalize_word(words[j + k]) {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    repeat_count += 1;
                    j += n;
                } else {
                    break;
                }
            }

            if repeat_count > 1 {
                // Keep only the first occurrence of the phrase
                output.extend_from_slice(&words[i..i + n]);
                i += n * repeat_count;
                collapsed = true;
                break;
            }
        }

        if !collapsed {
            output.push(words[i]);
            i += 1;
        }
    }

    output.join(" ")
}

/// Deduplicates repeated words and phrases in the text.
pub fn dedupe_repeated_phrases(text: &str) -> String {
    // First collapse multi-word repeated phrases
    let result = collapse_repeated_phrases(text);
    // Then collapse any remaining duplicate adjacent words
    collapse_duplicate_words(&result)
}

/// Main orchestrator function that applies all enabled post-processing steps.
pub fn apply_postprocessing(text: &str, settings: &Settings) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut result = text.to_string();

    // 1. Remove filler words first (before capitalization)
    if settings.remove_filler_words {
        result = remove_filler_words(&result, &settings.filler_words);
    }

    // 2. Apply custom replacements
    if !settings.custom_replacements.is_empty() {
        result = apply_replacements(&result, &settings.custom_replacements);
    }

    // 3. Remove punctuation
    if settings.remove_punctuation {
        result = remove_punctuation(&result);
    }

    // 4. Dedupe repeated phrases (like "I need you, I need you, I need you")
    if settings.dedupe_repeated_phrases {
        result = dedupe_repeated_phrases(&result);
    }

    // 5. Capitalize sentences last (so we capitalize the cleaned text)
    // Note: If punctuation is removed, this will only capitalize the first letter
    if settings.auto_capitalize {
        result = capitalize_sentences(&result);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capitalize_sentences() {
        assert_eq!(
            capitalize_sentences("hello. how are you?"),
            "Hello. How are you?"
        );
        assert_eq!(capitalize_sentences(""), "");
        assert_eq!(capitalize_sentences("test"), "Test");
        assert_eq!(
            capitalize_sentences("first. second! third?"),
            "First. Second! Third?"
        );
    }

    #[test]
    fn test_remove_filler_words() {
        let fillers = vec!["um".to_string(), "uh".to_string(), "like".to_string()];
        assert_eq!(
            remove_filler_words("So um I was like thinking", &fillers),
            "So I was thinking"
        );
        assert_eq!(remove_filler_words("uh hello", &fillers), "hello");
    }

    #[test]
    fn test_apply_replacements() {
        let rules = vec![
            ReplacementRule {
                find: "hte".to_string(),
                replace: "the".to_string(),
            },
            ReplacementRule {
                find: "teh".to_string(),
                replace: "the".to_string(),
            },
        ];
        assert_eq!(apply_replacements("hte quick fox", &rules), "the quick fox");
        assert_eq!(apply_replacements("teh dog", &rules), "the dog");
    }
}

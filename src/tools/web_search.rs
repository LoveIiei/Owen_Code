use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

pub struct WebSearch;

#[derive(Deserialize)]
struct DdgResponse {
    #[serde(rename = "AbstractText")]
    abstract_text: String,
    #[serde(rename = "AbstractSource")]
    abstract_source: String,
    #[serde(rename = "AbstractURL")]
    abstract_url: String,
    #[serde(rename = "RelatedTopics")]
    related_topics: Vec<RelatedTopic>,
    #[serde(rename = "Answer")]
    answer: String,
    #[serde(rename = "AnswerType")]
    answer_type: String,
    #[serde(rename = "Definition")]
    definition: String,
    #[serde(rename = "DefinitionSource")]
    definition_source: String,
}

#[derive(Deserialize)]
struct RelatedTopic {
    #[serde(rename = "Text")]
    text: Option<String>,
    #[serde(rename = "FirstURL")]
    first_url: Option<String>,
}

impl WebSearch {
    pub async fn search(query: &str) -> Result<String> {
        let client = Client::builder()
            .user_agent("aicode-tui/0.1 (terminal AI assistant)")
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            urlenccode(query)
        );

        let resp = client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("Search API returned {}", resp.status()));
        }

        let data: DdgResponse = resp.json().await?;

        let mut result = String::new();

        // Direct answer (e.g. math, conversions, facts)
        if !data.answer.is_empty() {
            result.push_str(&format!("**Answer:** {}\n\n", data.answer));
        }

        // Abstract (Wikipedia-style summary)
        if !data.abstract_text.is_empty() {
            result.push_str(&format!("**{}:** {}\n", data.abstract_source, data.abstract_text));
            if !data.abstract_url.is_empty() {
                result.push_str(&format!("Source: {}\n", data.abstract_url));
            }
            result.push('\n');
        }

        // Definition
        if !data.definition.is_empty() {
            result.push_str(&format!("**Definition ({}):** {}\n\n", data.definition_source, data.definition));
        }

        // Related topics (up to 5)
        let topics: Vec<_> = data
            .related_topics
            .iter()
            .filter_map(|t| {
                let text = t.text.as_deref().filter(|s| !s.is_empty())?;
                Some((text, t.first_url.as_deref().unwrap_or("")))
            })
            .take(5)
            .collect();

        if !topics.is_empty() {
            result.push_str("**Related:**\n");
            for (text, url) in topics {
                if url.is_empty() {
                    result.push_str(&format!("- {}\n", truncate(text, 120)));
                } else {
                    result.push_str(&format!("- {} ({})\n", truncate(text, 100), url));
                }
            }
        }

        if result.trim().is_empty() {
            // DDG returned nothing useful — fall back to a note
            result = format!(
                "No instant answer found for: \"{}\"\n\nTry a more specific query or check: https://duckduckgo.com/?q={}",
                query,
                urlenccode(query)
            );
        }

        Ok(result)
    }
}

fn urlenccode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            c => format!("%{:02X}", c as u32),
        })
        .collect()
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        // Truncate at char boundary
        let mut end = max;
        while !s.is_char_boundary(end) { end -= 1; }
        &s[..end]
    }
}

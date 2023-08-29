use anyhow::{anyhow, Result};
use futures::prelude::*;
use serde_json::{self, Value as Json};
use std::{env, fs, io::Write, thread, time::Duration};

const ALPHABETS: &[&str] = &include!("../../alphabets");

async fn query_words_by_char_domain(page: usize, domain: &str) -> reqwest::Result<String> {
    let url = reqwest::Url::parse_with_params(
        "https://dictionary.orst.go.th/Lookup/lookupDomain.php",
        &[("page", page.to_string()), ("domain", domain.to_string())],
    )
    .unwrap();
    reqwest::get(url).await?.text().await
}

fn extract_json_response(json: &Json) -> Option<(u64, Vec<String>)> {
    let words = json.as_array()?[1]
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(|s| s.to_string()))
        .collect::<Option<Vec<_>>>()?;
    let max_count = json.as_array()?[0].as_u64()?;
    Some((max_count, words))
}

#[derive(Debug)]
struct WordsPage {
    pub words: Vec<String>,
    pub max_count: usize,
}

impl WordsPage {
    const WORD_PER_PAGE: usize = 10;
    async fn query(page: usize, domain: &str) -> Result<WordsPage> {
        let text_response = query_words_by_char_domain(page, domain).await?;
        let json = serde_json::from_str(&text_response)?;
        let (max_count, words) = extract_json_response(&json).ok_or(anyhow!("invalid json"))?;
        Ok(WordsPage {
            words,
            max_count: max_count as usize,
        })
    }
    fn max_page(&self) -> usize {
        self.max_count / WordsPage::WORD_PER_PAGE + 1
    }
}

async fn words_in_domain(domain: &str, concurrent_requests: usize) -> Result<Vec<String>> {
    let first_page = WordsPage::query(1, domain).await?;
    let max_page = first_page.max_page();
    let rest_pages = stream::iter(2..=max_page)
        .map(|page_index| async move { WordsPage::query(page_index, domain).await })
        .buffer_unordered(concurrent_requests)
        .map(|result| result.expect("querying word: {}"))
        .collect::<Vec<WordsPage>>()
        .await;
    let words = first_page
        .words
        .into_iter()
        .chain(rest_pages.into_iter().flat_map(|w| w.words))
        .collect::<Vec<_>>();
    Ok(words)
}

async fn all_words<W: Write>(interval: Duration, mut writer: W) -> Result<()> {
    for alphabet in ALPHABETS {
        println!("querying for {alphabet}");
        let words = words_in_domain(alphabet, 50).await?;
        for word in words {
            writeln!(writer, "{word}")?;
        }
        println!("done. interval waiting...");
        thread::sleep(interval);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let output_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "all_words".to_string());
    let path = std::path::PathBuf::from(&output_path);
    let mut file = fs::File::options()
        .write(true)
        .create(true)
        .append(true)
        .open(&path)?;
    assert!(path.try_exists()?);
    all_words(Duration::from_secs(1), &mut file).await?;
    Ok(())
}

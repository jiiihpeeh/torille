//! A small CoNLL-U reader for the Phase 1 morphology coverage harness.
//!
//! Only the fields the harness needs are retained: surface form, lemma, UPOS
//! and the feature map. Multiword-token rows (`1-2`) and empty nodes (`1.1`)
//! are counted and skipped; syntactic words are the coverage unit. The reader
//! is deliberately independent of torille so it can be tested and reused by a
//! future training pipeline.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

/// One syntactic word from a CoNLL-U sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdToken {
    /// Integer ID within the sentence.
    pub id: usize,
    pub form: String,
    pub lemma: String,
    /// Universal POS, e.g. `NOUN`.
    pub upos: String,
    /// `FEATS` decoded into key/value pairs.
    pub feats: BTreeMap<String, String>,
    /// `HEAD` token id, when present.
    pub head: Option<usize>,
    /// Universal dependency relation.
    pub deprel: String,
}

impl UdToken {
    /// A token the tokeniser would treat as a word: at least one letter.
    pub fn is_word_like(&self) -> bool {
        self.form.chars().any(char::is_alphabetic)
    }

    pub fn feature(&self, key: &str) -> Option<&str> {
        self.feats.get(key).map(String::as_str)
    }
}

/// One CoNLL-U sentence.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UdSentence {
    /// `# sent_id` when present, otherwise `sent-{index}`.
    pub id: String,
    /// `# text` when present.
    pub text: Option<String>,
    pub tokens: Vec<UdToken>,
    /// Multiword-token rows skipped in this sentence.
    pub multiword_tokens: usize,
    /// Empty-node rows skipped in this sentence.
    pub empty_nodes: usize,
}

#[derive(Debug)]
pub enum UdError {
    Io { path: String, source: io::Error },
    Parse { line: usize, message: String },
}

impl fmt::Display for UdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UdError::Io { path, source } => write!(f, "{path}: {source}"),
            UdError::Parse { line, message } => write!(f, "line {line}: {message}"),
        }
    }
}

impl std::error::Error for UdError {}

impl From<io::Error> for UdError {
    fn from(source: io::Error) -> Self {
        UdError::Io {
            path: "<memory>".to_string(),
            source,
        }
    }
}

/// Read a CoNLL-U file.
pub fn read_file(path: impl AsRef<Path>) -> Result<Vec<UdSentence>, UdError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| UdError::Io {
        path: path.display().to_string(),
        source,
    })?;
    read_str(&text)
}

/// Parse CoNLL-U text. Blank lines end sentences; comments are metadata.
pub fn read_str(text: &str) -> Result<Vec<UdSentence>, UdError> {
    let mut sentences = Vec::new();
    let mut current = UdSentence::default();
    let mut current_lines = 0usize;

    for (line_index, raw) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() {
            finish_sentence(&mut sentences, &mut current, &mut current_lines);
            continue;
        }
        if let Some(comment) = line.strip_prefix('#') {
            let comment = comment.trim();
            if let Some(value) = comment.strip_prefix("sent_id") {
                current.id = value
                    .trim()
                    .strip_prefix('=')
                    .unwrap_or(value.trim())
                    .trim()
                    .to_string();
            } else if let Some(value) = comment.strip_prefix("text") {
                current.text = Some(
                    value
                        .trim()
                        .strip_prefix('=')
                        .unwrap_or(value.trim())
                        .trim()
                        .to_string(),
                );
            }
            continue;
        }
        current_lines += 1;

        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() < 10 {
            return Err(UdError::Parse {
                line: line_number,
                message: format!("expected 10 tab-separated columns, found {}", columns.len()),
            });
        }
        let id = columns[0];
        if id.contains('-') {
            current.multiword_tokens += 1;
            continue;
        }
        if id.contains('.') {
            current.empty_nodes += 1;
            continue;
        }
        let Ok(id) = id.parse::<usize>() else {
            return Err(UdError::Parse {
                line: line_number,
                message: format!("non-integer token id {id:?}"),
            });
        };
        current.tokens.push(UdToken {
            id,
            form: columns[1].to_string(),
            lemma: columns[2].to_string(),
            upos: columns[3].to_string(),
            feats: parse_feats(columns[5], line_number)?,
            head: columns[6].parse::<usize>().ok(),
            deprel: columns[7].to_string(),
        });
    }
    finish_sentence(&mut sentences, &mut current, &mut current_lines);
    Ok(sentences)
}

fn finish_sentence(
    sentences: &mut Vec<UdSentence>,
    current: &mut UdSentence,
    current_lines: &mut usize,
) {
    if *current_lines == 0 && current.tokens.is_empty() {
        // A comment-only block (metadata separated by blank lines) must not
        // leak its id/text into the next real sentence.
        *current = UdSentence::default();
        return;
    }
    if current.id.is_empty() {
        current.id = format!("sent-{}", sentences.len() + 1);
    }
    sentences.push(std::mem::take(current));
    *current_lines = 0;
}

fn parse_feats(raw: &str, line: usize) -> Result<BTreeMap<String, String>, UdError> {
    let mut feats = BTreeMap::new();
    if raw == "_" || raw.is_empty() {
        return Ok(feats);
    }
    for item in raw.split('|') {
        let Some((key, value)) = item.split_once('=') else {
            return Err(UdError::Parse {
                line,
                message: format!("malformed FEATS item {item:?}"),
            });
        };
        if key.is_empty() || value.is_empty() {
            return Err(UdError::Parse {
                line,
                message: format!("empty FEATS key/value in {item:?}"),
            });
        }
        feats.insert(key.to_string(), value.to_string());
    }
    Ok(feats)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# sent_id = s1
# text = Kissa istuu.
1\tKissa\tkissa\tNOUN\t_\tCase=Nom|Number=Sing\t2\tnsubj\t_\t_
2\tistuu\tistua\tVERB\t_\tMood=Ind|Person=3|Tense=Pres|VerbForm=Fin\t0\troot\t_\t_
3\t.\t.\tPUNCT\t_\t_\t2\tpunct\t_\t_
";

    #[test]
    fn reads_a_simple_sentence() {
        let sentences = read_str(SAMPLE).unwrap();
        assert_eq!(sentences.len(), 1);
        let sentence = &sentences[0];
        assert_eq!(sentence.id, "s1");
        assert_eq!(sentence.text.as_deref(), Some("Kissa istuu."));
        assert_eq!(sentence.tokens.len(), 3);
        assert_eq!(sentence.tokens[0].form, "Kissa");
        assert_eq!(sentence.tokens[0].feature("Case"), Some("Nom"));
        assert_eq!(sentence.tokens[1].feature("Mood"), Some("Ind"));
    }

    #[test]
    fn skips_multiword_and_empty_nodes() {
        let text = "\
# sent_id = s
1-2\tettei\t_\t_\t_\t_\t_\t_\t_\t_
1\tettä\tettä\tSCONJ\t_\t_\t2\tmark\t_\t_
2\tei\tei\tAUX\t_\tPolarity=Neg\t0\troot\t_\t_
2.1\t_\t_\t_\t_\t_\t_\t_\t_\t_
";
        let sentences = read_str(text).unwrap();
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0].tokens.len(), 2);
        assert_eq!(sentences[0].multiword_tokens, 1);
        assert_eq!(sentences[0].empty_nodes, 1);
    }

    #[test]
    fn a_malformed_row_is_an_error_with_a_line() {
        let error = read_str("1\tkissa\n").unwrap_err();
        assert!(error.to_string().contains("line 1"));
    }

    #[test]
    fn an_empty_input_is_empty_output() {
        assert!(read_str("\n\n").unwrap().is_empty());
    }

    #[test]
    fn comment_only_metadata_does_not_leak_into_the_next_sentence() {
        let text = "\
# sent_id = metadata-only
# text = ignored

# sent_id = real
# text = Kissa.
1\tKissa\tkissa\tNOUN\t_\tCase=Nom|Number=Sing\t0\troot\t_\t_
";
        let sentences = read_str(text).unwrap();
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0].id, "real");
        assert_eq!(sentences[0].text.as_deref(), Some("Kissa."));
    }
}

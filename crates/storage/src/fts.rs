use blockcell_core::{Error, Result};
use rusqlite::{Connection, OptionalExtension};

pub(crate) fn build_fts_query(query: &str) -> String {
    let mut latin = Vec::new();
    let mut cjk_terms = Vec::new();
    let mut cjk_run = String::new();

    let flush_cjk = |run: &mut String, terms: &mut Vec<String>| {
        let chars = run.chars().collect::<Vec<_>>();
        match chars.len() {
            0 => {}
            1 => terms.push(chars[0].to_string()),
            _ => terms.extend(chars.windows(2).map(|pair| pair.iter().collect::<String>())),
        }
        run.clear();
    };

    let mut latin_run = String::new();
    let flush_latin = |run: &mut String, terms: &mut Vec<String>| {
        if !run.is_empty() {
            terms.push(std::mem::take(run));
        }
    };

    for character in query.chars() {
        if is_cjk(character) {
            flush_latin(&mut latin_run, &mut latin);
            cjk_run.push(character);
        } else {
            flush_cjk(&mut cjk_run, &mut cjk_terms);
            if character.is_alphanumeric() || matches!(character, '-' | '_') {
                latin_run.push(character);
            } else {
                flush_latin(&mut latin_run, &mut latin);
            }
        }
    }
    flush_cjk(&mut cjk_run, &mut cjk_terms);
    flush_latin(&mut latin_run, &mut latin);

    if !cjk_terms.is_empty() {
        cjk_terms
            .into_iter()
            .chain(latin)
            .map(|term| format!("\"{}\"", term.replace('"', " ")))
            .collect::<Vec<_>>()
            .join(" OR ")
    } else if latin.is_empty() {
        "\"\"".to_string()
    } else {
        latin
            .into_iter()
            .map(|term| format!("\"{}\"", term.replace('"', " ")))
            .collect::<Vec<_>>()
            .join(" OR ")
    }
}

pub(crate) fn cjk_bigrams(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut run = Vec::new();
    let flush = |run: &mut Vec<char>, terms: &mut Vec<String>| {
        if run.len() == 1 {
            terms.push(run[0].to_string());
        } else {
            terms.extend(run.windows(2).map(|pair| pair.iter().collect::<String>()));
        }
        run.clear();
    };
    for character in query.chars() {
        if is_cjk(character) {
            run.push(character);
        } else {
            flush(&mut run, &mut terms);
        }
    }
    flush(&mut run, &mut terms);
    terms.sort();
    terms.dedup();
    terms
}

pub(crate) fn prepare_trigram_fts(
    conn: &Connection,
    table: &str,
    triggers: &[&str],
) -> Result<bool> {
    let schema = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(map_sqlite_error)?;
    let needs_rebuild = schema
        .as_deref()
        .is_none_or(|sql| !sql.to_ascii_lowercase().contains("tokenize='trigram'"));
    if schema.is_some() && needs_rebuild {
        for trigger in triggers {
            conn.execute_batch(&format!("DROP TRIGGER IF EXISTS {trigger};"))
                .map_err(map_sqlite_error)?;
        }
        conn.execute_batch(&format!("DROP TABLE IF EXISTS {table};"))
            .map_err(map_sqlite_error)?;
    }
    Ok(needs_rebuild)
}

fn is_cjk(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
    )
}

fn map_sqlite_error(error: rusqlite::Error) -> Error {
    Error::Storage(format!("FTS schema operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_query_expands_to_overlapping_bigrams() {
        assert_eq!(
            build_fts_query("发版检查什么"),
            "\"发版\" OR \"版检\" OR \"检查\" OR \"查什\" OR \"什么\""
        );
    }
}

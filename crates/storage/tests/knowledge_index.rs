use blockcell_core::Paths;
use blockcell_storage::KnowledgeIndex;
use blockcell_storage::{memory::UpsertParams, MemoryStore};

fn test_paths(label: &str) -> Paths {
    Paths::with_base(std::env::temp_dir().join(format!(
        "blockcell-knowledge-index-{label}-{}",
        uuid::Uuid::new_v4()
    )))
}

#[test]
fn rebuild_indexes_canonical_files_with_metadata() {
    let paths = test_paths("metadata");
    paths.ensure_dirs().expect("ensure paths");
    std::fs::write(
        paths.user_md(),
        "- [id:pref-reply] [scope:user] [source:user_statement] [updated:2026-08-01] 用户偏好简洁回答\n",
    )
    .expect("write USER.md");
    std::fs::write(
        paths.memory_md(),
        "## Project\n\n- [id:project-deploy] [scope:workspace] [source:verified] [updated:2026-07-30] 项目使用金丝雀发布。\n",
    )
    .expect("write MEMORY.md");

    let index = KnowledgeIndex::open(&paths.knowledge_index_db()).expect("open index");
    let result = index.rebuild_from_files(&paths).expect("rebuild index");

    assert_eq!(result.indexed, 2);
    let hits = index.search("用户偏好简洁回答", 10).expect("search index");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, "pref-reply");
    assert_eq!(hits[0].file, "USER.md");
    assert_eq!(hits[0].anchor, "root");
    assert_eq!(hits[0].scope, "user");
    assert_eq!(hits[0].source, "user_statement");
    assert_eq!(hits[0].updated_at, "2026-08-01");
    assert!(!hits[0].content_hash.is_empty());
    let conn = rusqlite::Connection::open(paths.knowledge_index_db()).expect("open raw index db");
    let vector_table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='knowledge_vectors'",
            [],
            |row| row.get(0),
        )
        .expect("query vector table");
    assert_eq!(vector_table_count, 1);
}

#[test]
fn rebuild_removes_entries_deleted_from_canonical_file() {
    let paths = test_paths("delete");
    paths.ensure_dirs().expect("ensure paths");
    std::fs::write(paths.user_md(), "User prefers concise release notes.\n")
        .expect("write USER.md");

    let index = KnowledgeIndex::open(&paths.knowledge_index_db()).expect("open index");
    index.rebuild_from_files(&paths).expect("initial rebuild");
    assert_eq!(
        index.search("concise", 10).expect("initial search").len(),
        1
    );

    std::fs::write(paths.user_md(), "").expect("clear USER.md");
    let result = index.rebuild_from_files(&paths).expect("second rebuild");

    assert_eq!(result.removed, 1);
    assert!(index
        .search("concise", 10)
        .expect("final search")
        .is_empty());
}

#[test]
fn legacy_long_term_rows_can_be_listed_and_retired_for_file_migration() {
    let paths = test_paths("legacy-long-term");
    paths.ensure_dirs().expect("ensure paths");
    let store = MemoryStore::open(&paths.memory_dir().join("memory.db")).expect("open memory");
    let long_term = store
        .upsert(UpsertParams {
            scope: "long_term".to_string(),
            item_type: "preference".to_string(),
            title: None,
            content: "User prefers concise replies.".to_string(),
            summary: None,
            tags: vec![],
            source: "legacy".to_string(),
            channel: None,
            session_key: None,
            importance: 0.8,
            dedup_key: None,
            expires_at: None,
        })
        .expect("seed long-term row");
    store
        .upsert(UpsertParams {
            scope: "short_term".to_string(),
            item_type: "note".to_string(),
            title: None,
            content: "Temporary task state.".to_string(),
            summary: None,
            tags: vec![],
            source: "legacy".to_string(),
            channel: None,
            session_key: Some("cli:test".to_string()),
            importance: 0.4,
            dedup_key: None,
            expires_at: None,
        })
        .expect("seed short-term row");

    let rows = store.active_long_term_items().expect("list long-term rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, long_term.id);

    assert_eq!(
        store
            .retire_long_term_items(&[long_term.id])
            .expect("retire rows"),
        1
    );
    assert!(store
        .active_long_term_items()
        .expect("list after retirement")
        .is_empty());
}

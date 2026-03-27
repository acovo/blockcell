use std::future::Future;
use std::sync::Arc;

use arrow_array::cast::AsArray;
use arrow_array::types::Float32Type;
use arrow_array::{
    FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray,
};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::{connect, Connection, Error as LanceError, Table};
use serde_json::json;
use tokio::runtime::{Handle, Runtime};
use tokio::task;

use crate::vector::{VectorHit, VectorIndex, VectorMeta};
use blockcell_core::{Error, Result};

const ID_COLUMN: &str = "id";
const VECTOR_COLUMN: &str = "vector";
const SCOPE_COLUMN: &str = "scope";
const ITEM_TYPE_COLUMN: &str = "item_type";
const TAGS_COLUMN: &str = "tags";

pub struct LanceDbIndex {
    runtime: Runtime,
    connection: Connection,
    table_name: String,
    cached_table: std::sync::Mutex<Option<Table>>,
    dimensions: std::sync::Mutex<Option<usize>>,
}

impl LanceDbIndex {
    pub fn open_or_create(uri: &str, table_name: &str) -> Result<Self> {
        let runtime = Runtime::new().map_err(|error| {
            Error::Storage(format!("Failed to create tokio runtime: {}", error))
        })?;
        let connection = runtime
            .block_on(connect(uri).execute())
            .map_err(map_lancedb_error)?;

        let table = match runtime.block_on(connection.open_table(table_name).execute()) {
            Ok(table) => Some(table),
            Err(LanceError::TableNotFound { .. }) => None,
            Err(error) => return Err(map_lancedb_error(error)),
        };
        let dimensions = if let Some(table) = table.as_ref() {
            let schema = runtime
                .block_on(table.schema())
                .map_err(map_lancedb_error)?;
            extract_vector_dimensions(&schema)
        } else {
            None
        };

        Ok(Self {
            runtime,
            connection,
            table_name: table_name.to_string(),
            cached_table: std::sync::Mutex::new(table),
            dimensions: std::sync::Mutex::new(dimensions),
        })
    }

    fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        if Handle::try_current().is_ok() {
            task::block_in_place(|| self.runtime.block_on(future))
        } else {
            self.runtime.block_on(future)
        }
    }

    fn ensure_table(&self, vector_dim: Option<usize>) -> Result<Option<Table>> {
        if let Some(table) = self
            .cached_table
            .lock()
            .map_err(|error| Error::Storage(format!("LanceDB table lock error: {}", error)))?
            .clone()
        {
            self.record_dimensions_from_table(&table)?;
            return Ok(Some(table));
        }

        match self.block_on(self.connection.open_table(&self.table_name).execute()) {
            Ok(table) => {
                self.record_dimensions_from_table(&table)?;
                *self.cached_table.lock().map_err(|error| {
                    Error::Storage(format!("LanceDB table lock error: {}", error))
                })? = Some(table.clone());
                return Ok(Some(table));
            }
            Err(LanceError::TableNotFound { .. }) => {}
            Err(error) => return Err(map_lancedb_error(error)),
        }

        let Some(vector_dim) = vector_dim else {
            return Ok(None);
        };

        let schema = vector_schema(vector_dim);
        let table = match self.block_on(
            self.connection
                .create_empty_table(&self.table_name, schema.clone())
                .execute(),
        ) {
            Ok(table) => table,
            Err(LanceError::TableAlreadyExists { .. }) => self
                .block_on(self.connection.open_table(&self.table_name).execute())
                .map_err(map_lancedb_error)?,
            Err(error) => return Err(map_lancedb_error(error)),
        };

        *self
            .cached_table
            .lock()
            .map_err(|error| Error::Storage(format!("LanceDB table lock error: {}", error)))? =
            Some(table.clone());
        *self.dimensions.lock().map_err(|error| {
            Error::Storage(format!("LanceDB dimension lock error: {}", error))
        })? = Some(vector_dim);

        Ok(Some(table))
    }

    fn record_dimensions_from_table(&self, table: &Table) -> Result<()> {
        let mut dimensions = self
            .dimensions
            .lock()
            .map_err(|error| Error::Storage(format!("LanceDB dimension lock error: {}", error)))?;
        if dimensions.is_none() {
            let schema = self.block_on(table.schema()).map_err(map_lancedb_error)?;
            *dimensions = extract_vector_dimensions(&schema);
        }
        Ok(())
    }

    fn ensure_matching_dimensions(&self, vector_dim: usize) -> Result<()> {
        let mut dimensions = self
            .dimensions
            .lock()
            .map_err(|error| Error::Storage(format!("LanceDB dimension lock error: {}", error)))?;
        match *dimensions {
            Some(expected) if expected != vector_dim => Err(Error::Storage(format!(
                "Vector dimension mismatch: expected {}, got {}",
                expected, vector_dim
            ))),
            Some(_) => Ok(()),
            None => {
                *dimensions = Some(vector_dim);
                Ok(())
            }
        }
    }
}

impl VectorIndex for LanceDbIndex {
    fn upsert(&self, id: &str, vector: &[f32], meta: &VectorMeta) -> Result<()> {
        self.ensure_matching_dimensions(vector.len())?;
        let Some(table) = self.ensure_table(Some(vector.len()))? else {
            return Ok(());
        };

        let batch = vector_record_batch(id, vector, meta)?;
        let schema = batch.schema();
        let reader = Box::new(RecordBatchIterator::new(
            vec![Ok(batch)].into_iter(),
            schema,
        ));
        let mut merge_insert = table.merge_insert(&[ID_COLUMN]);
        merge_insert
            .when_matched_update_all(None)
            .when_not_matched_insert_all();
        self.block_on(merge_insert.execute(reader))
            .map_err(map_lancedb_error)?;
        Ok(())
    }

    fn delete_ids(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        let Some(table) = self.ensure_table(None)? else {
            return Ok(());
        };
        let predicate = format!(
            "{} IN ({})",
            ID_COLUMN,
            ids.iter()
                .map(|id| format!("'{}'", escape_sql_string(id)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        self.block_on(table.delete(&predicate))
            .map_err(map_lancedb_error)?;
        Ok(())
    }

    fn search(&self, vector: &[f32], top_k: usize) -> Result<Vec<VectorHit>> {
        if top_k == 0 {
            return Ok(Vec::new());
        }

        let Some(table) = self.ensure_table(None)? else {
            return Ok(Vec::new());
        };
        self.ensure_matching_dimensions(vector.len())?;

        let mut stream = self.block_on(async {
            table
                .query()
                .nearest_to(vector)
                .map_err(map_lancedb_error)?
                .limit(top_k)
                .execute()
                .await
                .map_err(map_lancedb_error)
        })?;

        let mut hits = Vec::new();
        self.block_on(async {
            while let Some(batch) = stream.try_next().await.map_err(map_lancedb_error)? {
                hits.extend(record_batch_to_hits(&batch)?);
            }
            Ok::<(), Error>(())
        })?;

        Ok(hits)
    }

    fn health(&self) -> Result<()> {
        let _ = self
            .block_on(self.connection.table_names().execute())
            .map_err(map_lancedb_error)?;
        Ok(())
    }

    fn stats(&self) -> Result<serde_json::Value> {
        let table_names = self
            .block_on(self.connection.table_names().execute())
            .map_err(map_lancedb_error)?;
        if !table_names.iter().any(|name| name == &self.table_name) {
            return Ok(json!({
                "table": self.table_name,
                "exists": false,
                "rows": 0,
                "indices": 0,
            }));
        }

        let table = self.ensure_table(None)?.ok_or_else(|| {
            Error::Storage(format!(
                "LanceDB table {} is not available",
                self.table_name
            ))
        })?;
        let row_count = self
            .block_on(table.count_rows(None))
            .map_err(map_lancedb_error)?;
        let index_configs = self
            .block_on(table.list_indices())
            .map_err(map_lancedb_error)?;
        let table_stats = self.block_on(table.stats()).map_err(map_lancedb_error)?;

        Ok(json!({
            "table": self.table_name,
            "exists": true,
            "rows": row_count,
            "indices": index_configs.len(),
            "total_bytes": table_stats.total_bytes,
            "fragments": table_stats.fragment_stats.num_fragments,
            "small_fragments": table_stats.fragment_stats.num_small_fragments,
        }))
    }

    fn reset(&self) -> Result<()> {
        let table_names = self
            .block_on(self.connection.table_names().execute())
            .map_err(map_lancedb_error)?;
        if table_names.iter().any(|name| name == &self.table_name) {
            self.block_on(self.connection.drop_table(&self.table_name, &[]))
                .map_err(map_lancedb_error)?;
        }

        *self
            .cached_table
            .lock()
            .map_err(|error| Error::Storage(format!("LanceDB table lock error: {}", error)))? =
            None;
        *self.dimensions.lock().map_err(|error| {
            Error::Storage(format!("LanceDB dimension lock error: {}", error))
        })? = None;
        Ok(())
    }
}

fn vector_record_batch(id: &str, vector: &[f32], meta: &VectorMeta) -> Result<RecordBatch> {
    let schema = vector_schema(vector.len());
    let vector_values: Vec<Option<f32>> = vector.iter().copied().map(Some).collect();
    let vector_array = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
        vec![Some(vector_values)],
        vector.len() as i32,
    );
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec![id.to_string()])),
            Arc::new(vector_array),
            Arc::new(StringArray::from(vec![meta.scope.clone()])),
            Arc::new(StringArray::from(vec![meta.item_type.clone()])),
            Arc::new(StringArray::from(vec![meta.tags.join(",")])),
        ],
    )
    .map_err(|error| Error::Storage(format!("Failed to build LanceDB record batch: {}", error)))
}

fn vector_schema(vector_dim: usize) -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new(ID_COLUMN, DataType::Utf8, false),
        Field::new(
            VECTOR_COLUMN,
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                vector_dim as i32,
            ),
            false,
        ),
        Field::new(SCOPE_COLUMN, DataType::Utf8, false),
        Field::new(ITEM_TYPE_COLUMN, DataType::Utf8, false),
        Field::new(TAGS_COLUMN, DataType::Utf8, false),
    ]))
}

fn extract_vector_dimensions(schema: &Schema) -> Option<usize> {
    schema
        .field_with_name(VECTOR_COLUMN)
        .ok()
        .and_then(|field| {
            if let DataType::FixedSizeList(_, dimensions) = field.data_type() {
                Some(*dimensions as usize)
            } else {
                None
            }
        })
}

fn record_batch_to_hits(batch: &RecordBatch) -> Result<Vec<VectorHit>> {
    let ids = batch
        .column_by_name(ID_COLUMN)
        .ok_or_else(|| Error::Storage("LanceDB search result missing id column".to_string()))?
        .as_string::<i32>();
    let scores = batch.column_by_name("_distance").ok_or_else(|| {
        Error::Storage("LanceDB search result missing _distance column".to_string())
    })?;

    let distances: Vec<Option<f64>> =
        if let Some(values) = scores.as_any().downcast_ref::<Float32Array>() {
            values
                .iter()
                .map(|value| value.map(|value| value as f64))
                .collect()
        } else if let Some(values) = scores.as_any().downcast_ref::<arrow_array::Float64Array>() {
            values.iter().collect()
        } else {
            return Err(Error::Storage(
                "LanceDB search _distance column had unexpected type".to_string(),
            ));
        };

    let mut hits = Vec::new();
    for (id, distance) in ids.iter().zip(distances.into_iter()) {
        if let (Some(id), Some(distance)) = (id, distance) {
            hits.push(VectorHit {
                id: id.to_string(),
                score: -distance,
            });
        }
    }
    Ok(hits)
}

fn escape_sql_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn map_lancedb_error(error: LanceError) -> Error {
    Error::Storage(format!("LanceDB error: {}", error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_lancedb_index_smoke() {
        let dir = TempDir::new().unwrap();
        let uri = dir.path().join("vectors.lancedb");
        let index = LanceDbIndex::open_or_create(uri.to_str().unwrap(), "memory_vectors").unwrap();

        let meta = VectorMeta {
            scope: "long_term".to_string(),
            item_type: "fact".to_string(),
            tags: vec!["vector".to_string()],
        };

        index.upsert("memory-1", &[0.1, 0.2, 0.3], &meta).unwrap();
        let hits = index.search(&[0.1, 0.2, 0.3], 3).unwrap();
        assert!(hits.iter().any(|hit| hit.id == "memory-1"));

        index.upsert("memory-1", &[0.3, 0.2, 0.1], &meta).unwrap();
        let hits = index.search(&[0.3, 0.2, 0.1], 3).unwrap();
        assert!(hits.iter().any(|hit| hit.id == "memory-1"));
        let stats = index.stats().unwrap();
        assert_eq!(stats["exists"], true);
        assert_eq!(stats["rows"], 1);

        index.delete_ids(&["memory-1".to_string()]).unwrap();
        let hits = index.search(&[0.3, 0.2, 0.1], 3).unwrap();
        assert!(!hits.iter().any(|hit| hit.id == "memory-1"));

        index.reset().unwrap();
        let stats = index.stats().unwrap();
        assert_eq!(stats["exists"], false);
        let hits = index.search(&[0.3, 0.2, 0.1], 3).unwrap();
        assert!(hits.is_empty());
    }
}

use super::*;

impl MemoryStore {
    /// Soft-delete a memory item.
    pub fn soft_delete(&self, id: &str) -> Result<bool> {
        let deleted = {
            let mut conn = self
                .inner
                .lock()
                .map_err(|e| blockcell_core::Error::Storage(format!("Lock error: {}", e)))?;
            let tx = conn.transaction().map_err(|e| {
                blockcell_core::Error::Storage(format!(
                    "Begin soft-delete transaction error: {}",
                    e
                ))
            })?;
            let now = Utc::now().to_rfc3339();
            let affected = tx
                .execute(
                    "UPDATE memory_items SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
                    params![now, id],
                )
                .map_err(|e| blockcell_core::Error::Storage(format!("Soft delete error: {}", e)))?;
            if affected > 0 && self.vector.is_some() {
                Self::enqueue_vector_sync_on_conn(&tx, id, VECTOR_SYNC_OP_DELETE, 0, None)?;
            }
            tx.commit().map_err(|e| {
                blockcell_core::Error::Storage(format!(
                    "Commit soft-delete transaction error: {}",
                    e
                ))
            })?;
            affected > 0
        };

        if deleted {
            self.sync_vector_delete(id);
        }

        Ok(deleted)
    }

    /// Batch soft-delete by filter criteria.
    pub fn batch_soft_delete(
        &self,
        scope: Option<&str>,
        item_type: Option<&str>,
        tags: Option<&[String]>,
        time_before: Option<&str>,
    ) -> Result<usize> {
        let ids = {
            let mut conn = self
                .inner
                .lock()
                .map_err(|e| blockcell_core::Error::Storage(format!("Lock error: {}", e)))?;
            let tx = conn.transaction().map_err(|e| {
                blockcell_core::Error::Storage(format!(
                    "Begin batch-delete transaction error: {}",
                    e
                ))
            })?;

            let mut sql = "SELECT id FROM memory_items WHERE deleted_at IS NULL".to_string();
            let mut bind_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            let mut idx = 1;

            if let Some(s) = scope {
                sql.push_str(&format!(" AND scope = ?{}", idx));
                bind_values.push(Box::new(s.to_string()));
                idx += 1;
            }
            if let Some(t) = item_type {
                sql.push_str(&format!(" AND type = ?{}", idx));
                bind_values.push(Box::new(t.to_string()));
                idx += 1;
            }
            if let Some(tag_list) = tags {
                if !tag_list.is_empty() {
                    let mut tag_conditions = Vec::new();
                    for tag in tag_list {
                        tag_conditions.push(format!("tags LIKE '%' || ?{} || '%'", idx));
                        bind_values.push(Box::new(tag.clone()));
                        idx += 1;
                    }
                    sql.push_str(&format!(" AND ({})", tag_conditions.join(" OR ")));
                }
            }
            if let Some(before) = time_before {
                sql.push_str(&format!(" AND created_at < ?{}", idx));
                bind_values.push(Box::new(before.to_string()));
            }

            let bind_refs: Vec<&dyn rusqlite::types::ToSql> =
                bind_values.iter().map(|b| b.as_ref()).collect();
            let mut stmt = tx.prepare(&sql).map_err(|e| {
                blockcell_core::Error::Storage(format!("Batch delete prepare error: {}", e))
            })?;
            let rows = stmt
                .query_map(bind_refs.as_slice(), |row| row.get::<_, String>(0))
                .map_err(|e| {
                    blockcell_core::Error::Storage(format!("Batch delete select error: {}", e))
                })?;

            let mut ids = Vec::new();
            for row in rows {
                ids.push(row.map_err(|e| {
                    blockcell_core::Error::Storage(format!("Batch delete id row error: {}", e))
                })?);
            }

            if ids.is_empty() {
                drop(stmt);
                tx.commit().map_err(|e| {
                    blockcell_core::Error::Storage(format!(
                        "Commit batch-delete transaction error: {}",
                        e
                    ))
                })?;
                return Ok(0);
            }

            drop(stmt);

            let now = Utc::now().to_rfc3339();
            let placeholders = (0..ids.len())
                .map(|offset| format!("?{}", offset + 2))
                .collect::<Vec<_>>()
                .join(", ");
            let update_sql = format!(
                "UPDATE memory_items SET deleted_at = ?1 WHERE id IN ({})",
                placeholders
            );

            let mut update_values: Vec<Box<dyn rusqlite::types::ToSql>> =
                Vec::with_capacity(ids.len() + 1);
            update_values.push(Box::new(now));
            for id in &ids {
                update_values.push(Box::new(id.clone()));
            }
            let update_refs: Vec<&dyn rusqlite::types::ToSql> =
                update_values.iter().map(|value| value.as_ref()).collect();

            tx.execute(&update_sql, update_refs.as_slice())
                .map_err(|e| {
                    blockcell_core::Error::Storage(format!("Batch delete update error: {}", e))
                })?;

            if self.vector.is_some() {
                for id in &ids {
                    Self::enqueue_vector_sync_on_conn(&tx, id, VECTOR_SYNC_OP_DELETE, 0, None)?;
                }
            }
            tx.commit().map_err(|e| {
                blockcell_core::Error::Storage(format!(
                    "Commit batch-delete transaction error: {}",
                    e
                ))
            })?;

            ids
        };

        self.sync_vector_delete_ids(&ids);
        info!(count = ids.len(), "Batch soft-deleted memory items");
        Ok(ids.len())
    }

    /// Restore a soft-deleted item.
    pub fn restore(&self, id: &str) -> Result<bool> {
        let restored_item = {
            let mut conn = self
                .inner
                .lock()
                .map_err(|e| blockcell_core::Error::Storage(format!("Lock error: {}", e)))?;
            let tx = conn.transaction().map_err(|e| {
                blockcell_core::Error::Storage(format!("Begin restore transaction error: {}", e))
            })?;
            let affected = tx
                .execute(
                    "UPDATE memory_items SET deleted_at = NULL WHERE id = ?1 AND deleted_at IS NOT NULL",
                    params![id],
                )
                .map_err(|e| blockcell_core::Error::Storage(format!("Restore error: {}", e)))?;

            if affected == 0 {
                tx.commit().map_err(|e| {
                    blockcell_core::Error::Storage(format!(
                        "Commit restore transaction error: {}",
                        e
                    ))
                })?;
                None
            } else {
                let item = self.get_by_id_inner(&tx, id)?;
                if self.vector.is_some() {
                    Self::enqueue_vector_sync_on_conn(&tx, id, VECTOR_SYNC_OP_UPSERT, 0, None)?;
                }
                tx.commit().map_err(|e| {
                    blockcell_core::Error::Storage(format!(
                        "Commit restore transaction error: {}",
                        e
                    ))
                })?;
                Some(item)
            }
        };

        if let Some(item) = restored_item {
            self.sync_vector_upsert(&item);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Clean up expired items (set deleted_at) and hard-delete items that have been
    /// soft-deleted for more than `recycle_days` days.
    pub fn maintenance(&self, recycle_days: i64) -> Result<(usize, usize)> {
        let (expired_ids, purged_ids) = {
            let mut conn = self
                .inner
                .lock()
                .map_err(|e| blockcell_core::Error::Storage(format!("Lock error: {}", e)))?;
            let tx = conn.transaction().map_err(|e| {
                blockcell_core::Error::Storage(format!(
                    "Begin maintenance transaction error: {}",
                    e
                ))
            })?;

            let now = Utc::now().to_rfc3339();
            let cutoff = (Utc::now() - chrono::Duration::days(recycle_days)).to_rfc3339();

            let expired_ids = {
                let mut stmt = tx
                    .prepare(
                        "SELECT id FROM memory_items
                         WHERE expires_at IS NOT NULL
                           AND expires_at <= ?1
                           AND deleted_at IS NULL",
                    )
                    .map_err(|e| {
                        blockcell_core::Error::Storage(format!("TTL cleanup prepare error: {}", e))
                    })?;
                let rows = stmt
                    .query_map(params![now], |row| row.get::<_, String>(0))
                    .map_err(|e| {
                        blockcell_core::Error::Storage(format!("TTL cleanup query error: {}", e))
                    })?;
                let mut ids = Vec::new();
                for row in rows {
                    ids.push(row.map_err(|e| {
                        blockcell_core::Error::Storage(format!("TTL cleanup id row error: {}", e))
                    })?);
                }
                ids
            };

            let purged_ids = {
                let mut stmt = tx
                    .prepare(
                        "SELECT id FROM memory_items
                         WHERE deleted_at IS NOT NULL
                           AND deleted_at < ?1",
                    )
                    .map_err(|e| {
                        blockcell_core::Error::Storage(format!("Purge prepare error: {}", e))
                    })?;
                let rows = stmt
                    .query_map(params![cutoff], |row| row.get::<_, String>(0))
                    .map_err(|e| {
                        blockcell_core::Error::Storage(format!("Purge query error: {}", e))
                    })?;
                let mut ids = Vec::new();
                for row in rows {
                    ids.push(row.map_err(|e| {
                        blockcell_core::Error::Storage(format!("Purge id row error: {}", e))
                    })?);
                }
                ids
            };

            if !expired_ids.is_empty() {
                let placeholders = (0..expired_ids.len())
                    .map(|offset| format!("?{}", offset + 2))
                    .collect::<Vec<_>>()
                    .join(", ");
                let sql = format!(
                    "UPDATE memory_items SET deleted_at = ?1 WHERE id IN ({})",
                    placeholders
                );
                let mut values: Vec<Box<dyn rusqlite::types::ToSql>> =
                    Vec::with_capacity(expired_ids.len() + 1);
                values.push(Box::new(now));
                for id in &expired_ids {
                    values.push(Box::new(id.clone()));
                }
                let refs: Vec<&dyn rusqlite::types::ToSql> =
                    values.iter().map(|value| value.as_ref()).collect();
                tx.execute(&sql, refs.as_slice()).map_err(|e| {
                    blockcell_core::Error::Storage(format!("TTL cleanup update error: {}", e))
                })?;
            }

            if !purged_ids.is_empty() {
                let placeholders = (0..purged_ids.len())
                    .map(|offset| format!("?{}", offset + 1))
                    .collect::<Vec<_>>()
                    .join(", ");
                let sql = format!("DELETE FROM memory_items WHERE id IN ({})", placeholders);
                let values: Vec<&dyn rusqlite::types::ToSql> = purged_ids
                    .iter()
                    .map(|id| id as &dyn rusqlite::types::ToSql)
                    .collect();
                tx.execute(&sql, values.as_slice()).map_err(|e| {
                    blockcell_core::Error::Storage(format!("Purge delete error: {}", e))
                })?;
            }

            if self.vector.is_some() {
                for id in expired_ids.iter().chain(purged_ids.iter()) {
                    Self::enqueue_vector_sync_on_conn(&tx, id, VECTOR_SYNC_OP_DELETE, 0, None)?;
                }
            }
            tx.commit().map_err(|e| {
                blockcell_core::Error::Storage(format!(
                    "Commit maintenance transaction error: {}",
                    e
                ))
            })?;

            (expired_ids, purged_ids)
        };

        let mut deleted_ids = expired_ids.clone();
        deleted_ids.extend(purged_ids.iter().cloned());
        self.sync_vector_delete_ids(&deleted_ids);

        if !expired_ids.is_empty() || !purged_ids.is_empty() {
            info!(
                expired = expired_ids.len(),
                purged = purged_ids.len(),
                "Memory maintenance completed"
            );
        }

        Ok((expired_ids.len(), purged_ids.len()))
    }

    pub fn retry_vector_sync(&self, limit: usize) -> Result<VectorSyncRetryResult> {
        if self.vector.is_none() {
            return Err(blockcell_core::Error::Storage(
                "Vector runtime is not enabled".to_string(),
            ));
        }

        let pending = self.load_pending_vector_sync(limit)?;
        let mut result = VectorSyncRetryResult {
            attempted: pending.len(),
            succeeded: 0,
            failed: 0,
        };

        for entry in pending {
            let sync_result = match entry.operation.as_str() {
                VECTOR_SYNC_OP_DELETE => {
                    self.try_vector_delete_ids(std::slice::from_ref(&entry.id))
                }
                VECTOR_SYNC_OP_UPSERT => match self.get_by_id(&entry.id)? {
                    Some(item) if is_item_active_for_vector(&item) => self.try_vector_upsert(&item),
                    _ => self.try_vector_delete_ids(std::slice::from_ref(&entry.id)),
                },
                other => Err(blockcell_core::Error::Storage(format!(
                    "Unknown vector sync operation: {}",
                    other
                ))),
            };

            match sync_result {
                Ok(()) => {
                    self.clear_vector_sync(&entry.id);
                    result.succeeded += 1;
                }
                Err(error) => {
                    self.enqueue_vector_sync(&entry.id, &entry.operation, &error.to_string());
                    warn!(
                        id = %entry.id,
                        operation = %entry.operation,
                        error = %error,
                        "Retrying vector sync failed"
                    );
                    result.failed += 1;
                }
            }
        }

        Ok(result)
    }

    pub fn reindex_vectors(&self) -> Result<VectorReindexResult> {
        let runtime = self.vector.as_ref().ok_or_else(|| {
            blockcell_core::Error::Storage("Vector runtime is not enabled".to_string())
        })?;

        runtime.index.reset()?;
        self.clear_all_vector_sync()?;

        let items = self.load_reindexable_items()?;
        let mut result = VectorReindexResult {
            indexed: 0,
            failed: 0,
        };

        for item in items {
            match self.try_vector_upsert(&item) {
                Ok(()) => {
                    self.clear_vector_sync(&item.id);
                    result.indexed += 1;
                }
                Err(error) => {
                    self.enqueue_vector_sync(&item.id, VECTOR_SYNC_OP_UPSERT, &error.to_string());
                    warn!(id = %item.id, error = %error, "Failed to reindex vector entry");
                    result.failed += 1;
                }
            }
        }

        Ok(result)
    }
}

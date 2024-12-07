use rusqlite::{Connection, Result, params};
use std::path::Path;
use std::sync::mpsc::{channel, Sender};
use std::thread;

pub struct Tool {
    pub id: i64,
    pub name: String,
}

pub struct TimingPerTransaction {
    pub id: i64,
    pub transaction_signature: String,
    pub tool_id: i64,
    pub timestamp: i64,
}

pub struct TokenCreation {
    pub id: i64,
    pub name: String,
    pub mint: String,
    pub transaction_signature: String,
    pub detect_tool_id: i64,
}

enum DbOperation {
    InsertTokenCreation {
        name: String,
        mint: String,
        transaction_signature: String,
        detect_tool_id: i64,
    },
    InsertTiming {
        transaction_signature: String,
        tool_id: i64,
        timestamp: i64,
    },
}

pub struct Database {
    conn: Connection,
    operation_sender: Option<Sender<DbOperation>>,
}

impl Database {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path.as_ref())?;
        
        // Create tables if they don't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS Tool (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
            )",
            [],
        )?;

        // Insert default tools if they don't exist
        conn.execute(
            "INSERT OR IGNORE INTO Tool (id, name) VALUES (1, 'shredstream')",
            [],
        )?;
        
        conn.execute(
            "INSERT OR IGNORE INTO Tool (id, name) VALUES (2, 'helius_yellowstone')",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS TimingPerTransaction (
                id INTEGER PRIMARY KEY,
                transaction_signature TEXT NOT NULL,
                tool_id INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY (tool_id) REFERENCES Tool(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS TokenCreations (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                mint TEXT NOT NULL,
                transaction_signature TEXT NOT NULL,
                detect_tool_id INTEGER NOT NULL,
                FOREIGN KEY (detect_tool_id) REFERENCES Tool(id)
            )",
            [],
        )?;

        // Create a background worker thread for database operations
        let (tx, rx) = channel();
        let db_path = path.as_ref().to_path_buf();
        
        thread::spawn(move || {
            let worker_conn = Connection::open(&db_path).expect("Failed to open DB connection in worker");
            
            while let Ok(operation) = rx.recv() {
                match operation {
                    DbOperation::InsertTokenCreation { name, mint, transaction_signature, detect_tool_id } => {
                        let _ = worker_conn.execute(
                            "INSERT INTO TokenCreations (name, mint, transaction_signature, detect_tool_id)
                             VALUES (?1, ?2, ?3, ?4)",
                            params![name, mint, transaction_signature, detect_tool_id],
                        );
                    },
                    DbOperation::InsertTiming { transaction_signature, tool_id, timestamp } => {
                        let _ = worker_conn.execute(
                            "INSERT INTO TimingPerTransaction (transaction_signature, tool_id, timestamp)
                             VALUES (?1, ?2, ?3)",
                            params![transaction_signature, tool_id, timestamp],
                        );
                    }
                }
            }
        });

        Ok(Database { 
            conn,
            operation_sender: Some(tx),
        })
    }

    // Tool operations
    pub fn insert_tool(&self, name: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO Tool (name) VALUES (?1)",
            params![name],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_tool(&self, id: i64) -> Result<Option<Tool>> {
        let mut stmt = self.conn.prepare("SELECT id, name FROM Tool WHERE id = ?1")?;
        let mut tool_iter = stmt.query_map(params![id], |row| {
            Ok(Tool {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        
        tool_iter.next().transpose()
    }

    pub fn get_tool_by_name(&self, name: &str) -> Result<Option<Tool>> {
        let mut stmt = self.conn.prepare("SELECT id, name FROM Tool WHERE name = ?1")?;
        let mut tool_iter = stmt.query_map(params![name], |row| {
            Ok(Tool {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        
        tool_iter.next().transpose()
    }

    // TimingPerTransaction operations
    pub fn insert_timing_async(
        &self,
        transaction_signature: String,
        tool_id: i64,
        timestamp: i64,
    ) {
        if let Some(sender) = &self.operation_sender {
            let _ = sender.send(DbOperation::InsertTiming {
                transaction_signature,
                tool_id,
                timestamp,
            });
        }
    }

    pub fn get_timing(&self, id: i64) -> Result<Option<TimingPerTransaction>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, transaction_signature, tool_id, timestamp 
             FROM TimingPerTransaction WHERE id = ?1"
        )?;
        
        let mut timing_iter = stmt.query_map(params![id], |row| {
            Ok(TimingPerTransaction {
                id: row.get(0)?,
                transaction_signature: row.get(1)?,
                tool_id: row.get(2)?,
                timestamp: row.get(3)?,
            })
        })?;
        
        timing_iter.next().transpose()
    }

    pub fn get_timings_by_tool(&self, tool_id: i64) -> Result<Vec<TimingPerTransaction>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, transaction_signature, tool_id, timestamp 
             FROM TimingPerTransaction WHERE tool_id = ?1"
        )?;
        
        let timing_iter = stmt.query_map(params![tool_id], |row| {
            Ok(TimingPerTransaction {
                id: row.get(0)?,
                transaction_signature: row.get(1)?,
                tool_id: row.get(2)?,
                timestamp: row.get(3)?,
            })
        })?;
        
        timing_iter.collect()
    }

    // TokenCreation operations
    pub fn insert_token_creation_async(
        &self,
        name: String,
        mint: String,
        transaction_signature: String,
        detect_tool_id: i64,
    ) {
        if let Some(sender) = &self.operation_sender {
            let _ = sender.send(DbOperation::InsertTokenCreation {
                name,
                mint,
                transaction_signature,
                detect_tool_id,
            });
        }
    }

    pub fn get_token_creation(&self, id: i64) -> Result<Option<TokenCreation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, mint, transaction_signature, detect_tool_id 
             FROM TokenCreations WHERE id = ?1"
        )?;
        
        let mut token_iter = stmt.query_map(params![id], |row| {
            Ok(TokenCreation {
                id: row.get(0)?,
                name: row.get(1)?,
                mint: row.get(2)?,
                transaction_signature: row.get(3)?,
                detect_tool_id: row.get(4)?,
            })
        })?;
        
        token_iter.next().transpose()
    }

    pub fn get_token_creation_by_mint(&self, mint: &str) -> Result<Option<TokenCreation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, mint, transaction_signature, detect_tool_id 
             FROM TokenCreations WHERE mint = ?1"
        )?;
        
        let mut token_iter = stmt.query_map(params![mint], |row| {
            Ok(TokenCreation {
                id: row.get(0)?,
                name: row.get(1)?,
                mint: row.get(2)?,
                transaction_signature: row.get(3)?,
                detect_tool_id: row.get(4)?,
            })
        })?;
        
        token_iter.next().transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_database_operations() -> Result<()> {
        let db_path = "test_analytics.db";
        
        // Clean up any existing test database
        let _ = fs::remove_file(db_path);
        
        let db = Database::new(db_path)?;
        
        // Test Tool operations
        let tool_id = db.insert_tool("test_tool")?;
        let tool = db.get_tool(tool_id)?.unwrap();
        assert_eq!(tool.name, "test_tool");
        
        // Test async operations
        db.insert_token_creation_async(
            "Test Token".to_string(),
            "test_mint".to_string(),
            "test_sig".to_string(),
            1,
        );

        // Give some time for the async operation to complete
        thread::sleep(Duration::from_millis(100));

        // Verify the async insertion
        let token = db.get_token_creation_by_mint("test_mint")?.unwrap();
        assert_eq!(token.name, "Test Token");
        assert_eq!(token.mint, "test_mint");
        assert_eq!(token.transaction_signature, "test_sig");
        assert_eq!(token.detect_tool_id, 1);
        
        // Clean up
        fs::remove_file(db_path).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Ok(())
    }
}

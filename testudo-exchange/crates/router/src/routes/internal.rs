//! JNL-SYNC-01 CP-6: Internal admin endpoints removed.
//!
//! The WS-driven journal write path has been deleted. The JournalSyncer
//! (pull-based) is now the sole authority for journal data.

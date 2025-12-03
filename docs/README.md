# Documentation Index

This directory contains architecture and design documentation for the Intuition event processing system.

## 📚 Documents Overview

### [ARCHITECTURE_PROPOSAL.md](./ARCHITECTURE_PROPOSAL.md)
**System Architecture Specification: High-Scale Blockchain Event Processing**

A production-ready architecture proposal implementing **Event Sourcing** and **CQRS** patterns for processing blockchain events at scale.

**Key Topics:**
- Event Sourcing architecture with ordered event store
- Three decoupled projection streams (Metadata, User State, Market Data)
- Sharded parallelism using "Sticky Partitioning" for 4M+ positions
- TimescaleDB with compression policies for 100GB storage constraint
- High availability ingestion with leader election
- Dead Letter Queue (DLQ) for fault tolerance
- Reorg handling strategies

**Target Scale:** 300k Vaults, 4M+ Positions, 300+ EPS (Peak)  
**Stack:** Rust, PostgreSQL (TimescaleDB), Kubernetes

---

### [SYSTEM_ARCHITECTURE_CURRENT.md](./SYSTEM_ARCHITECTURE_CURRENT.md)
**Current System Architecture: Event Processing Pipeline**

Documentation of the existing multi-stage consumer pipeline architecture.

**Key Topics:**
- Three-stage consumer pipeline: Decoded → Resolver → IpfsUpload
- Event processing flow from raw logs to enriched data
- Database schema and table structures
- Trigger-based aggregate maintenance
- Consumer modes and their responsibilities
- Current limitations and scaling considerations

**Use Case:** Understanding the current implementation before migration to Event Sourcing architecture.

---

### [BACKEND_REQUIREMENTS.md](./BACKEND_REQUIREMENTS.md)
**Backend Feature Requirements**

Tracking document for backend feature requirements and enhancements.

**Key Topics:**
- Real-time event streaming for frontend (replacing polling)
- Time series API for share price data
- Developer experience improvements

**Use Case:** Planning and tracking backend feature development.

---

## 🗺️ Navigation Guide

**New to the system?** Start with [SYSTEM_ARCHITECTURE_CURRENT.md](./SYSTEM_ARCHITECTURE_CURRENT.md) to understand the current implementation.

**Planning a migration?** Review [ARCHITECTURE_PROPOSAL.md](./ARCHITECTURE_PROPOSAL.md) for the proposed Event Sourcing architecture.


---

## 📝 Document Status

| Document | Status | Last Updated |
|----------|--------|--------------|
| ARCHITECTURE_PROPOSAL.md | Production-Ready Spec | v3.0 |
| SYSTEM_ARCHITECTURE_CURRENT.md | Current System Docs | Active |
| BACKEND_REQUIREMENTS.md | Active | 2025-01-XX |

---

## 🔗 Related Documentation

- **CLI Documentation:** See `apps/cli/README.md` and `apps/cli/AGENTS.md`
- **Consumer Implementation:** See `apps/consumer/README.md`
- **Migration Scripts:** See `scripts/` directory for database migration utilities


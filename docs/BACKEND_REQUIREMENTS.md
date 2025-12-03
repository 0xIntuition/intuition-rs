# Backend Requirements

This document tracks feature requirements and enhancements for the backend services.

**Status:** Active  
**Last Updated:** 2025-01-XX

---

## 📋 Requirements

### 1. Real-Time Event Streaming for Frontend

**Status:** 🔴 Needs Discussion  
**Priority:** High

**Requirement:**
Enable the frontend to listen for events from the backend in real-time instead of polling.

**Details:**
- Replace polling mechanism with event streaming (WebSocket/SSE/Server-Sent Events)
- Frontend should be able to subscribe to specific event types
- Backend should push events to connected clients as they occur

**Open Questions:**
- What events does the FE need to track in real-time?
  - Share price changes?
  - Position updates?
  - Vault state changes?
  - Transaction confirmations?
  - Other events?
- What is the expected event volume per client?
- Do we need filtering/querying capabilities on the event stream?
- Should events be persisted for replay or ephemeral only?

**Technical Considerations:**
- WebSocket vs Server-Sent Events (SSE) vs WebRTC
- Authentication/authorization for event streams
- Connection management and reconnection handling
- Rate limiting and backpressure
- Event ordering guarantees

---

### 2. Time Series API for Share Price Data

**Status:** 🟡 In Planning  
**Priority:** High

**Requirement:**
Provide an API endpoint that returns time series data for share price changes, with intervals filled for periods where no changes occurred. This enables the frontend to plot SVGs/charts from the data.

**Details:**
- API should accept time range parameters (start time, end time, interval/granularity)
- Fill missing intervals with the last known value (forward fill) or appropriate interpolation
- Support multiple vaults/shares in a single request
- Optimize for chart rendering use cases

**API Considerations:**
- Endpoint structure: `/api/v1/shares/{vault_id}/price-history?start={timestamp}&end={timestamp}&interval={duration}`
- Response format: JSON array of `{timestamp, price, ...}` objects
- Interval options: 1m, 5m, 15m, 1h, 4h, 1d, etc.
- Handling edge cases:
  - No data in range
  - Sparse data (long gaps)
  - Multiple price changes within an interval (aggregation strategy)

**Data Source:**
- Query from existing share price tracking tables
- Consider TimescaleDB continuous aggregates for performance
- Cache frequently requested ranges

---

### 3. Developer Experience: Easy Backend Setup

**Status:** 🟡 In Planning  
**Priority:** Medium

**Requirement:**
Make the backend easy to run for developers, reducing setup friction and onboarding time.

**Details:**
- Streamline local development setup
- Provide clear documentation
- Automate common setup tasks
- Ensure consistent environment across developers

**Specific Improvements:**
- [ ] Single command to start all required services (docker-compose, makefile, etc.)
- [ ] Clear setup instructions in README
- [ ] Pre-configured development environment (Docker, localstack, etc.)
- [ ] Seed data scripts for local testing
- [ ] Environment variable templates (.env.example)
- [ ] Health check endpoints for all services
- [ ] Logging configuration that's useful for development
- [ ] Hot reload/watch mode for development
- [ ] Database migration automation
- [ ] Mock/test data generators

**Current State:**
- Review existing setup scripts and documentation
- Identify pain points in current developer onboarding
- Document required external dependencies

---

## 📝 Notes

- This document should be updated as requirements are clarified and implemented
- Each requirement should track:
  - Status (Needs Discussion, In Planning, In Progress, Completed, Blocked)
  - Priority (High, Medium, Low)
  - Dependencies
  - Implementation notes

---

## 🔗 Related Documentation

- [ARCHITECTURE_PROPOSAL.md](./ARCHITECTURE_PROPOSAL.md) - System architecture
- [SYSTEM_ARCHITECTURE_CURRENT.md](./SYSTEM_ARCHITECTURE_CURRENT.md) - Current implementation details


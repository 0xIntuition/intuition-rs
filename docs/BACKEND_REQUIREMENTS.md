# Backend Requirements

This document tracks feature requirements and enhancements for the backend services.

**Status:** Active  
**Last Updated:** 2025-12-03

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

### 4. Season 2 Airdrop Features

**Status:** 🔴 Needs Discussion  
**Priority:** High

**Requirement:**
Backend support for Season 2 Airdrop program, including enhanced data tracking, PNL calculations, and economic game mechanics.

**Details:**

#### 4.1 Oppose/Support Data Enhancement
- Make oppose and support actions more useful and trackable
- Provide data insights on oppose/support patterns
- Enable analysis of user behavior around these actions

#### 4.2 Portal & Economic Game Mechanics
**Context:** Portal is much larger than username, making economic games less compelling. Need to:
- Make clear the behaviors we are trying to incentivize
- Direct people to contribute and curate along specific guidelines
- Concentrate monetary activity in certain areas
- Improve early adopter experience

**Requirements:**
- Flexible system to direct users to focus areas (implementation-agnostic):
  - Specific lists
  - Specific tags
  - Specific sets of skills
  - Sections of the knowledge graph (when available)
  - Other targeting mechanisms as needed
- API endpoints to query and filter by these focus areas
- Tracking of user activity by focus area
- Metrics for monetary activity concentration
- Behavioral incentive tracking

**Note:** Since we're not using latent space yet, this requirement should be characterized broadly to allow flexibility in how we solve the problem. The goal is to enable directing users to contribute/curate along specific guidelines, regardless of the underlying mechanism.

#### 4.3 Fee Tracking & Trading Volume
- Track all fees associated with positions and transactions
- Provide fee breakdown by transaction type
- Support fee history queries
- Calculate total fees paid per user/vault/position
- **Trading Volume:** Track and display trading volume metrics
  - Trading volume shows the same results as protocol fees but presents bigger numbers from the end user's perspective
  - Makes the platform feel more impressive and engaging
  - Support volume queries per user/vault/position/time period

#### 4.4 PNL (Profit & Loss) Calculations
- **Realized PNL:** Calculate PNL for closed positions
- **Unrealized PNL:** Calculate current PNL for open positions
- **PNL Granularity:**
  - Global PNL (across all positions)
  - Atom-specific PNL
  - Triple-specific PNL
- Support historical PNL queries

#### 4.5 Position Timeseries Data
- Track when positions were opened
- Track when positions were closed (if applicable)
- Current status of positions (open/closed)
- Position lifecycle events timeline
- Support queries for position history over time ranges

**Note:** Time-series data work for Season 2 Airdrop will also serve:
- (1) User portfolio tracking
- (2) Trending markets for explore
- (3) Unlock leaderboards

**API Considerations:**
- Endpoints for PNL queries (realized, unrealized, by scope)
- Position timeseries endpoints
- Fee tracking and reporting endpoints
- Trading volume endpoints
- Focus area activity endpoints (flexible targeting mechanism)
- Behavioral metrics endpoints

**Data Requirements:**
- Position open/close timestamps
- Fee tracking tables/aggregates
- Trading volume tracking and aggregates
- PNL calculation logic (realized vs unrealized)
- Focus area mapping (lists, tags, skills, knowledge graph sections, etc.)
- User activity tracking by focus area

**Open Questions:**
- How should PNL be calculated? (mark-to-market, cost basis, etc.)
- What time granularity is needed for position timeseries?
- What specific focus areas should be prioritized? (lists, tags, skills, knowledge graph sections, etc.)
- What specific behaviors should be tracked for incentives?
- How to handle partial position closes for PNL calculations?
- How should trading volume be calculated and aggregated?

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


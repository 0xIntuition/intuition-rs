## **Relevance Scoring Formula**

The search requirement "most relevant items first" needs a concrete, justifiable, and tunable formula that converts the current state of an Atom or Triple into a single **Relevance Score**. This score will be the primary sorting mechanism for the search API.

**Proposed Relevance Score (R)**
We propose a formula that combines **Magnitude** (the amount of capital deposited) and **Recency** (how recently the item was active), scaled by a tunable factor (**α**).

$R = (\text{Net Position Magnitude}) \times (\text{Recency Factor})$

**A. Net Position Magnitude (M)**
This is a measure of the total current support/opposition expressed by users.

$$
M = (\sum \text{Total Assets in Support Vault}) + (\sum \text{Total Assets in Oppose Vault})
$$

• **Rationale:** The simple sum of total assets reflects the overall economic commitment to the statement, regardless of sentiment. An item with more value deposited is, by definition, more relevant to the community.
• **Engineering Implementation:** This value is derived directly from the aggregated `totalAssets` field stored in the database, updated by the Indexing Service.

**B. Recency Factor *Fr***
This factor ensures that recently active Triples/Atoms are prioritized over equally large but dormant ones. We use an **Exponential Decay Function** where *t* is the time elapsed since the last relevant event.

$$
F_r = 1 + \alpha \cdot e^{-t/\tau}

$$

Where:
• ***t***: Time (in days or hours) since the last `SharePriceChange` event on the Triple/Atom.
• **α (Recency Weight)**: A tunable parameter that controls the *maximum boost* recent activity gives (e.g., if **α**=0.2, the score can be boosted by up to 20%).
• *T* **(Half-life)**: A tunable time constant that determines how quickly the boost decays (e.g., if *T*=7 days, the boost is halved after 7 days).
• **Rationale:** This smoothly decaying function prevents sudden drops in relevance and allows product managers to tune how quickly items fall off the "Trending" list.
• **Engineering Implementation:** The Indexing Service must track and update the `lastActivityTimestamp` on every `SharePriceChange` event. The API Service calculates *Fr* at query time using the current time and this timestamp.

**2.2 Final Formula**

The final Relevance Score *R* for sorting:
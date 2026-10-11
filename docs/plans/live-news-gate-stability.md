# Live news gate stability plan

## Purpose

Keep the live native gate focused on whether the configured model can produce a
current sourced news answer through the Panel.

## Steps

1. Direct current-news requests to `web_search`.
2. Reserve `web_fetch` and its approval path for requests that name a page or
   URL.
3. Make the live news test fail if it receives a page-fetch approval request.
4. Keep approval coverage in the deterministic native test suite.
5. Run static checks and dispatch the native gate for PR #29.

## Done condition

The live test completes a sourced news answer without entering the page-fetch
approval flow, and the deterministic test continues to cover approval
decisions.

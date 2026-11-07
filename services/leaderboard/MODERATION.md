# Leaderboard moderation policy

The OpenUsage leaderboard ranks totals that people report from their own local logs. The service checks the shape and the limits of each upload, but it cannot prove any total (OU-BOARD-011). Every view says Self-reported.

## What maintainers review

- **Flagged days.** A user day above 2 billion tokens leaves the ranks at once. A maintainer looks at it and clears the flag only when the total looks possible.
- **Reports.** Anyone can report an entry. Open an issue in the OpenUsage repository with the GitHub login and the reason. Never paste a token, a log or a screenshot of a credential.

## What maintainers can do

- Hide an entry from the ranks.
- Clear a flag after a review.
- Delete a user and all their rows when the user asks, or when an entry breaks this policy.

Maintainers make these changes with the D1 command line under their Cloudflare login. No web endpoint can make them.

## What maintainers never do

- Change a reported value.
- Publish any string from an upload. Public views show only the GitHub login, ranks, token totals and the estimated value.
- Keep request logs, IP addresses or GitHub tokens.

## What breaks this policy

- Totals from scripts, test data or any source other than the user's own local logs.
- A GitHub account made to fill the ranks.
- An attempt to overload the service or to get past its limits.

## Leaving and appeals

Anyone can leave at any time with Leave in the app or `openusage board leave`. Leaving deletes the user, every device and every row at once (OU-BOARD-010). To appeal a decision, comment on its issue. A maintainer answers there.

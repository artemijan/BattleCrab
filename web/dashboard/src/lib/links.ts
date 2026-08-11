/**
 * Off-site destinations, in one place.
 *
 * The repository URL is spelled out in the footer, in the landing page's
 * feedback notice and (indirectly) in the issue links below. Renaming the repo
 * with those copies scattered leaves a half-broken site, so they all derive
 * from `GITHUB_URL`.
 */
export const GITHUB_URL = "https://github.com/artemijan/BattleCrab";

/** The issue list — what has already been reported, and what is being worked on. */
export const GITHUB_ISSUES_URL = `${GITHUB_URL}/issues`;

/**
 * Straight to the blank issue form.
 *
 * Not `/issues/new/choose`: there are no issue templates in the repo, and that
 * path only makes sense once there are. GitHub sends a signed-out visitor
 * through the login screen and back here, so the link works either way.
 */
export const GITHUB_NEW_ISSUE_URL = `${GITHUB_URL}/issues/new`;

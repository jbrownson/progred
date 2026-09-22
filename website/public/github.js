async function loadGitHubCount() {
  try {
    const response = await fetch("https://api.github.com/repos/jbrownson/progred", {
      credentials: "omit",
      signal: AbortSignal.timeout(5000),
    });
    if (!response.ok) return;
    const { stargazers_count: stars } = await response.json();
    if (!Number.isSafeInteger(stars) || stars < 0) return;
    const count = document.querySelector("#github-count");
    count.textContent = stars.toLocaleString();
    count.hidden = false;
    const label = `View progred on GitHub (${count.textContent} stars)`;
    const link = document.querySelector(".github-link");
    link.setAttribute("aria-label", label);
    link.title = label;
  } catch {
    // The repository link works even when the optional count is unavailable.
  }
}

loadGitHubCount();

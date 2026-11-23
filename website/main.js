const REPO = "karimawi/Klue";

const FILES = {
  windows: { suffix: "-windows-x64.zip", name: "Windows" },
  "mac-arm": { suffix: "-macos-arm64.dmg", name: "macOS" },
  "mac-intel": { suffix: "-macos-x64.dmg", name: "macOS" },
  deb: { suffix: "-linux-x64.deb", name: "Linux" },
  tar: { suffix: "-linux-x64.tar.gz", name: "Linux" },
};

function platform() {
  const ua = navigator.userAgent;
  const os = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "";
  if (/android|iphone|ipad|ipod/i.test(ua)) return null;
  if (/win/i.test(os) || /windows/i.test(ua)) return "windows";
  if (/mac/i.test(os) || /mac os x/i.test(ua)) return "mac-arm";
  if (/linux/i.test(os) || /linux/i.test(ua)) return "deb";
  return null;
}

const mine = platform();
const hero = document.querySelector("[data-hero-download]");

if (mine) {
  hero.querySelector("span").textContent = `Download for ${FILES[mine].name}`;
  document.querySelector(`.dl[data-file="${mine}"]`)?.classList.add("mine");
}

// Point the buttons at the newest release's files (pre-releases included).
fetch(`https://api.github.com/repos/${REPO}/releases?per_page=10`, { headers: { Accept: "application/vnd.github+json" } })
  .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
  .then((releases) => {
    const release = releases.find((r) => !r.draft && r.assets.length);
    if (!release) return;
    const version = `v${release.tag_name.replace(/^v/, "")}${release.prerelease ? " beta" : ""}`;
    document.querySelectorAll("[data-version]").forEach((el) => (el.textContent = version));
    for (const [key, file] of Object.entries(FILES)) {
      const asset = release.assets.find((a) => a.name.endsWith(file.suffix));
      const url = asset ? asset.browser_download_url : release.html_url;
      document.querySelectorAll(`.dl[data-file="${key}"]`).forEach((el) => (el.href = url));
      if (key === mine && asset) hero.href = url;
    }
  })
  .catch(() => {});

// Sections slide in as they come into view.
const reveal = new IntersectionObserver(
  (entries) => {
    for (const e of entries) {
      if (e.isIntersecting) {
        e.target.classList.add("in");
        reveal.unobserve(e.target);
      }
    }
  },
  { rootMargin: "0px 0px -8% 0px", threshold: 0.08 },
);
document.querySelectorAll(".reveal").forEach((el) => reveal.observe(el));

// A line under the nav once the page scrolls.
const nav = document.querySelector(".nav");
const onScroll = () => nav.classList.toggle("scrolled", window.scrollY > 8);
window.addEventListener("scroll", onScroll, { passive: true });
onScroll();

// Gallery arrows.
const gallery = document.querySelector(".gallery");
document.querySelectorAll("[data-scroll]").forEach((button) => {
  button.addEventListener("click", () => {
    const step = gallery.querySelector("figure").getBoundingClientRect().width + 28;
    gallery.scrollBy({ left: step * Number(button.dataset.scroll), behavior: "smooth" });
  });
});

document.querySelectorAll("[data-year]").forEach((el) => (el.textContent = new Date().getFullYear()));

function dom(tag, attrs = {}, ...children) {
  const el = document.createElement(tag);

  const isDangerousAttr = (name) =>
    /^on/i.test(name) || name === "srcdoc" || name === "innerHTML";

  for (const [k, v] of Object.entries(attrs)) {
    if (isDangerousAttr(k)) continue;

    if (k === "class") {
      el.className = v;
    } else if (k === "style" && v && typeof v === "object") {
      for (const [prop, val] of Object.entries(v)) {
        const str = String(val);
        if (/url\s*\(\s*javascript:/i.test(str)) continue;
        if (/^\d+px$/.test(str) && +str.slice(0, -2) > 10_000) continue;
        el.style[prop] = str;
      }
    } else if (k === "dataset" && v && typeof v === "object") {
      for (const [dk, dv] of Object.entries(v)) {
        if (/^[\w-]+$/.test(dk)) el.dataset[dk] = dv;
      }
    } else {
      el.setAttribute(k, v);
    }
  }

  // children / text
  for (const child of children) {
    el.append(child instanceof Node ? child : document.createTextNode(child));
  }
  return el;
}

function formatFromNow(timestamp) {
  const now = new Date();
  const date = new Date(timestamp.replace(" ", "T") + "Z");
  const diff = Math.floor((now - date) / 1000);
  const rtf = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

  if (Math.abs(diff) < 60) return rtf.format(-diff, "second");
  if (Math.abs(diff) < 3600) return rtf.format(-(diff / 60) | 0, "minute");
  if (Math.abs(diff) < 86400) return rtf.format(-(diff / 3600) | 0, "hour");
  return rtf.format(-(diff / 86400) | 0, "day");
}

function prettyPrintTimeDifference(t1, t2) {
  const d1 = new Date(t1),
    d2 = new Date(t2);
  if (isNaN(d1) || isNaN(d2)) return ["00", "00", "00"];
  const s = (Math.abs(d2 - d1) / 1000) | 0;
  const hh = (s / 3600) | 0;
  const mm = ((s % 3600) / 60) | 0;
  const ss = s % 60;
  return [hh, mm, ss].map((n) => String(n).padStart(2, "0"));
}

function mapHourlyEventsToLocalTime(events) {
  const now = new Date();
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);

  const hours = Array.from({ length: 24 }, (_, i) => {
    const hDate = new Date(start.getTime() + i * 3600 * 1000);
    let h = hDate.getHours();
    const ampm = h >= 12 ? "PM" : "AM";
    h = h % 12 || 12;
    return {
      formattedHour: `${h}${ampm}`,
      hour: hDate.getHours(),
      date: hDate.toISOString().slice(0, 10),
      count: 0,
      isCurrent:
        now.getHours() === hDate.getHours() &&
        now.getDate() === hDate.getDate(),
    };
  });

  events.forEach((e) => {
    const utc = new Date(e.hour + "Z");
    const loc = new Date(
      utc.getTime() +
        utc.getTimezoneOffset() * 60000 +
        now.getTimezoneOffset() * -60000
    );
    const idx = hours.findIndex(
      (h) =>
        h.date === loc.toISOString().slice(0, 10) && h.hour === loc.getHours()
    );
    if (idx !== -1) hours[idx].count += Number(e.count) || 0;
  });

  return hours;
}

function buildTableCard(titleLeft, titleRight, rows) {
  return dom(
    "div",
    { class: "tablecard" },
    dom(
      "div",
      { class: "top" },
      dom("div", { class: "left" }, titleLeft),
      dom("div", { class: "right" }, titleRight)
    ),
    ...rows
  );
}

/* --------------------------------------------------------------------------
   Hourly bar chart
   -------------------------------------------------------------------------- */
async function renderHourlySummary() {
  const data = await fetch("/summary/hourly").then((r) => r.json());
  const hours = mapHourlyEventsToLocalTime(data);
  const max = Math.max(...hours.map((h) => h.count));
  const scale = max ? 150 / max : 0;
  let pastCurrent = false;

  const chart = dom("div", { class: "hourly" });

  hours.forEach((h) => {
    const safeCount = Number.isFinite(h.count) ? h.count : 0;

    const barFill = dom("div", {
      class: "bar-fill",
      style: { height: `${(safeCount * scale) | 0}px` },
    });

    const bar = dom(
      "div",
      {
        class: `bar ${pastCurrent ? "future" : ""}`,
        style: { height: "150px" },
      },
      barFill
    );

    if (h.isCurrent) pastCurrent = true;

    chart.append(
      dom(
        "div",
        { class: "col" },
        bar,
        dom(
          "div",
          { class: `hour ${h.isCurrent ? "current" : ""}` },
          h.formattedHour
        )
      )
    );
  });

  document.getElementById("hourly").replaceChildren(chart);
}

/* --------------------------------------------------------------------------
   Top paths
   -------------------------------------------------------------------------- */
async function renderUrls() {
  const urls = await fetch("/summary/urls").then((r) => r.json());

  const rows = urls.map(({ url, count }) => {
    let host = "",
      path = url;
    try {
      const u = new URL(url);
      host = u.host;
      path = u.pathname + u.search;
    } catch {
      /* fallback */
    }

    return dom(
      "div",
      { class: "item" },
      dom(
        "div",
        { class: "left" },
        dom("div", { class: "time" }, String(count))
      ),
      dom(
        "div",
        { class: "right" },
        dom("div", { class: "host" }, host),
        dom("div", { class: "path" }, path)
      )
    );
  });

  document
    .getElementById("urls")
    .replaceChildren(buildTableCard("Top paths", "Last 7 days", rows));
}

/* --------------------------------------------------------------------------
   Top browsers
   -------------------------------------------------------------------------- */
async function renderBrowsers() {
  const container = document.getElementById("browsers");
  if (!container) return;

  const list = await fetch("/summary/osbrowsers").then((r) => r.json());

  const rows = list.map(({ os, browser, count }) =>
    dom(
      "div",
      { class: "item" },
      dom(
        "div",
        { class: "left" },
        dom("div", { class: "time" }, String(count))
      ),
      dom(
        "div",
        { class: "right" },
        dom("div", { class: "host" }, os),
        dom("div", { class: "path" }, browser)
      )
    )
  );

  container.replaceChildren(
    buildTableCard("Top browsers", "Last 7 days", rows)
  );
}

/* --------------------------------------------------------------------------
   Top referrers
   -------------------------------------------------------------------------- */
async function renderReferrers() {
  const refs = await fetch("/summary/referrers").then((r) => r.json());

  const rows = refs.map(({ domain, count }) =>
    dom(
      "div",
      { class: "item" },
      dom(
        "div",
        { class: "left" },
        dom("div", { class: "time" }, String(count))
      ),
      dom(
        "div",
        { class: "right" },
        dom("div", { class: "host" }, domain.replace(/\/$/, ""))
      )
    )
  );

  document
    .getElementById("referrers")
    .replaceChildren(buildTableCard("Top referrers", "Last 7 days", rows));
}

/* --------------------------------------------------------------------------
   Sessions list
   -------------------------------------------------------------------------- */
async function renderSessions() {
  const sessions = await fetch("/sessions").then((r) => r.json());
  const wrap = dom("div", { class: "sessions" });

  sessions.forEach((sess) => {
    const dur = prettyPrintTimeDifference(
      sess.events[0]?.timestamp,
      sess.events[sess.events.length - 1]?.timestamp
    );

    const header = dom(
      "div",
      { class: "top" },
      dom(
        "div",
        { class: "left" },
        `${sess.events?.length} event${sess.events?.length !== 1 ? "s" : ""}` +
          ` → from ${sess.collector.city}, ${sess.collector.country}`
      ),
      dom(
        "div",
        { class: "right" },
        dom(
          "div",
          { class: "duration" },
          ...dur.map((d, i) =>
            dom("div", { class: "item" }, d, dom("b", {}, ["H", "M", "S"][i]))
          )
        )
      )
    );

    const evList = dom(
      "div",
      { class: "events" },
      ...sess.events.map((ev) => {
        let host = "",
          path = ev.url;
        try {
          const u = new URL(ev.url);
          host = u.host;
          path = u.pathname + u.search;
        } catch {
          /* fallback */
        }
        return dom(
          "div",
          { class: "event" },
          dom(
            "div",
            { class: "left" },
            dom("div", { class: "name" }, ev.name),
            dom("div", { class: "host" }, host),
            dom("div", { class: "path" }, path)
          ),
          dom(
            "div",
            { class: "right" },
            dom("div", { class: "time" }, formatFromNow(ev.timestamp))
          )
        );
      })
    );

    wrap.append(dom("div", { class: "session" }, header, evList));
  });

  document.getElementById("sessions").replaceChildren(wrap);
}

/* --------------------------------------------------------------------------
   Summary numbers & percentage chips
   -------------------------------------------------------------------------- */
async function renderSummary() {
  const sum = await fetch("/summary").then((r) => r.json());
  Object.keys(sum).forEach((k) => {
    const el = document.getElementById(k);
    if (el) el.textContent = sum[k];
  });
}

function renderSinglePercentageChange(id, pct) {
  const el = document.getElementById(id);
  if (!el) return;
  el.classList.remove("pos", "neg");

  let txt = "-";
  if (pct < 0) {
    el.classList.add("neg");
    txt = `↓${((Math.abs(pct) * 10) | 0) / 10}%`;
  } else if (pct > 0) {
    el.classList.add("pos");
    txt = `↑${((pct * 10) | 0) / 10}%`;
  } else {
    txt = "0%";
  }

  el.textContent = txt;
}

async function renderPercentageChanges() {
  const p = await fetch("/summary/percentages").then((r) => r.json());
  renderSinglePercentageChange("pDay", p.day);
  renderSinglePercentageChange("pWeek", p.week);
  renderSinglePercentageChange("pMonth", p.month);
}

/* --------------------------------------------------------------------------
   Header clock
   -------------------------------------------------------------------------- */
async function renderHeader() {
  const now = new Date();
  const opts = {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
    hour: "numeric",
    minute: "numeric",
  };
  document.getElementById("headerTime").textContent = now.toLocaleDateString(
    "en-US",
    opts
  );
}

function convertUtcToLocal(utcDay, utcHour, offset) {
  let localHour = utcHour - offset;
  let localDay = utcDay;
  if (localHour < 0) {
    localHour += 24;
    localDay = (localDay + 6) % 7;
  } else if (localHour >= 24) {
    localHour -= 24;
    localDay = (localDay + 1) % 7;
  }
  return { localDay, localHour };
}

/* --------------------------------------------------------------------------
   Weekly heat-map
   -------------------------------------------------------------------------- */
async function renderWeeklyHeatmap() {
  const utc = await fetch("/summary/weekly").then((r) => r.json());
  const off = new Date().getTimezoneOffset() / 60;

  const adj = utc.map((e) => {
    let h = e.hour - off,
      d = e.day;
    if (h < 0) {
      h += 24;
      d = (d + 6) % 7;
    } else if (h >= 24) {
      h -= 24;
      d = (d + 1) % 7;
    }
    return { ...e, day: d, hour: h | 0 };
  });

  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const map = dom("div");

  for (let d = 0; d < 7; d++) {
    const row = dom(
      "div",
      { class: "day-row" },
      dom("div", { class: "day-name" }, days[d])
    );

    for (let h = 0; h < 24; h++) {
      const ev = adj.find((e) => e.day === d && e.hour === h);
      const cnt = ev ? ev.count : 0;
      const a = Math.min(cnt / 100, 1);
      row.append(
        dom("div", {
          class: "hour-cell",
          style: { backgroundColor: cnt ? `rgba(255,79,51,${a})` : "#222" },
          title: `${days[d]} ${h}:00 – ${cnt} events`,
        })
      );
    }
    map.append(row);
  }

  document.getElementById("weekly").replaceChildren(map);
}

/* --------------------------------------------------------------------------
   Rotating globe
   -------------------------------------------------------------------------- */
let world; // Globe.js instance (cached)

async function renderGlobe() {
  const coords = await fetch("/sessions/map").then((r) => r.json());

  /* leaderboard */
  const lb = coords
    .slice()
    .sort((a, b) => b.size - a.size)
    .slice(0, 12)
    .map((c) => dom("div", { class: "city" }, c.city));
  document.getElementById("globeleaderboard").replaceChildren(...lb);

  /* 3-D globe */
  if (!world) {
    world = Globe()
      .width(600)
      .backgroundColor("#111")
      .atmosphereColor("#999")
      .enablePointerInteraction(false)
      .globeImageUrl("third-party/earth-dark.jpg")
      .pointAltitude("size")
      .pointColor("color")(document.getElementById("globe"));

    world.controls().autoRotate = true;
    world.controls().autoRotateSpeed = 1;
  }

  /* clamp incoming values */
  const safeCoords = coords.map((c) => ({
    ...c,
    size: Math.min(Math.max(+c.size || 0.01, 0.01), 0.5),
    color: /^#?[0-9a-f]{3,8}$/i.test(c.color) ? c.color : "#ff4f33",
  }));
  world.pointsData(safeCoords);
}

/* --------------------------------------------------------------------------
   Bootstrapping & live refresh
   -------------------------------------------------------------------------- */
async function fetchAndRenderAnalytics() {
  try {
    await Promise.all([
      renderHeader(),
      renderPercentageChanges(),
      renderSessions(),
      renderSummary(),
      renderHourlySummary(),
      renderUrls(),
      renderBrowsers(),
      renderReferrers(),
      renderWeeklyHeatmap(),
      renderGlobe(),
    ]);
  } catch (e) {
    console.error("Error fetching analytics:", e);
  }
}

function refreshAnalytics() {
  if (document.hidden) return;
  fetchAndRenderAnalytics();

  const live = document.getElementById("live");
  live.style.backgroundColor = "red";
  setTimeout(() => {
    live.style.backgroundColor = "#e2e2e2";
    live.classList.remove("fresh");
  }, 9000);
}

/* initial paint & schedule */
fetchAndRenderAnalytics();
setInterval(refreshAnalytics, 10_000);
document.addEventListener("visibilitychange", refreshAnalytics);

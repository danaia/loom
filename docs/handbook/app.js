const canvas = document.querySelector("#agent-canvas");
const ctx = canvas.getContext("2d");
const lab = document.querySelector(".hero-lab");
const countLabel = document.querySelector(".lab-count");
let width = 0;
let height = 0;
let dpr = 1;
let mode = "organism";
let agents = [];
let frame = 0;
let animationId;

const palettes = {
  organism: { agent: "#9fe6c9", leader: "#ff7557", line: "#9fe6c9", field: "#d9ff62" },
  crystal: { agent: "#d9ff62", leader: "#ffffff", line: "#78c7bc", field: "#9fe6c9" },
  swarm: { agent: "#c9d2ff", leader: "#d9ff62", line: "#6b89ff", field: "#ff7557" },
};

function sizeCanvas() {
  const rect = canvas.getBoundingClientRect();
  dpr = Math.min(window.devicePixelRatio || 1, 2);
  width = rect.width;
  height = rect.height;
  canvas.width = width * dpr;
  canvas.height = height * dpr;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
}

function seededNoise(seed) {
  const x = Math.sin(seed * 999.91) * 43758.5453;
  return x - Math.floor(x);
}

function makeAgents() {
  agents = [];
  const total = mode === "crystal" ? 96 : mode === "swarm" ? 68 : 82;
  for (let i = 0; i < total; i += 1) {
    const angle = i * 2.39996;
    const spread = Math.sqrt(i / total);
    agents.push({
      angle,
      spread,
      phase: seededNoise(i + 8) * Math.PI * 2,
      speed: 0.003 + seededNoise(i + 30) * 0.005,
      size: i === 0 ? 6 : 2.2 + seededNoise(i + 50) * 2.5,
    });
  }
  frame = 0;
}

function agentPosition(agent, index, t) {
  const cx = width / 2;
  const cy = height / 2 - 6;
  const maxR = Math.min(width, height) * 0.35;
  if (mode === "crystal") {
    const ring = Math.floor(Math.sqrt(index));
    const points = Math.max(1, ring * 6);
    const a = (index % points) / points * Math.PI * 2;
    const r = ring * 10.5;
    const facet = 1 + 0.14 * Math.cos(6 * a);
    return {
      x: cx + Math.cos(a) * r * facet,
      y: cy + Math.sin(a) * r * facet * 0.82,
    };
  }
  if (mode === "swarm") {
    const flow = t * agent.speed * 90 + agent.phase;
    return {
      x: cx + Math.cos(flow * 0.7 + agent.angle) * maxR * (0.35 + agent.spread * 0.65),
      y: cy + Math.sin(flow + agent.angle * 0.65) * maxR * (0.25 + agent.spread * 0.6),
    };
  }
  const pulse = 1 + Math.sin(t * agent.speed * 18 + agent.phase) * 0.04;
  const r = maxR * agent.spread * pulse;
  return {
    x: cx + Math.cos(agent.angle + Math.sin(t * 0.0002 + agent.phase) * 0.07) * r,
    y: cy + Math.sin(agent.angle) * r * 0.73 + Math.sin(agent.phase + t * 0.001) * 2,
  };
}

function drawField(t, palette) {
  const cx = width / 2;
  const cy = height / 2 - 6;
  const gradient = ctx.createRadialGradient(cx, cy, 4, cx, cy, Math.min(width, height) * 0.43);
  gradient.addColorStop(0, `${palette.field}20`);
  gradient.addColorStop(0.55, `${palette.field}0c`);
  gradient.addColorStop(1, `${palette.field}00`);
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 54, width, height - 102);
  ctx.strokeStyle = "rgba(255,255,255,.055)";
  ctx.lineWidth = 1;
  for (let r = 55; r < Math.min(width, height) * 0.45; r += 45) {
    ctx.beginPath();
    ctx.ellipse(cx, cy, r, r * 0.72, 0, 0, Math.PI * 2);
    ctx.stroke();
  }
  ctx.strokeStyle = `${palette.field}28`;
  ctx.beginPath();
  ctx.arc(cx, cy, 34 + Math.sin(t * 0.002) * 4, 0, Math.PI * 2);
  ctx.stroke();
}

function draw(t) {
  ctx.clearRect(0, 0, width, height);
  const palette = palettes[mode];
  drawField(t, palette);
  const positions = agents.map((agent, index) => agentPosition(agent, index, t));

  ctx.lineWidth = 0.55;
  for (let i = 0; i < positions.length; i += 1) {
    for (let j = i + 1; j < positions.length; j += 1) {
      const dx = positions[i].x - positions[j].x;
      const dy = positions[i].y - positions[j].y;
      const dist = Math.hypot(dx, dy);
      if (dist < (mode === "crystal" ? 18 : 38)) {
        ctx.strokeStyle = `${palette.line}${Math.round((1 - dist / 38) * 36).toString(16).padStart(2, "0")}`;
        ctx.beginPath();
        ctx.moveTo(positions[i].x, positions[i].y);
        ctx.lineTo(positions[j].x, positions[j].y);
        ctx.stroke();
      }
    }
  }

  positions.forEach((position, index) => {
    const agent = agents[index];
    ctx.beginPath();
    ctx.arc(position.x, position.y, agent.size, 0, Math.PI * 2);
    ctx.fillStyle = index === 0 ? palette.leader : palette.agent;
    ctx.globalAlpha = index === 0 ? 1 : 0.48 + agent.spread * 0.48;
    ctx.fill();
    if (index === 0) {
      ctx.strokeStyle = palette.leader;
      ctx.globalAlpha = 0.35;
      ctx.beginPath();
      ctx.arc(position.x, position.y, 12 + Math.sin(t * 0.004) * 2, 0, Math.PI * 2);
      ctx.stroke();
    }
  });
  ctx.globalAlpha = 1;

  frame += 1;
  const visible = Math.min(agents.length, Math.max(1, Math.floor(frame / 3)));
  countLabel.innerHTML = `<strong>${visible.toLocaleString()}</strong> ${mode === "swarm" ? "agents" : "cells"}`;
  animationId = requestAnimationFrame(draw);
}

function setMode(nextMode) {
  mode = nextMode;
  lab.dataset.simulation = mode;
  document.querySelectorAll("[data-mode]").forEach((button) => {
    button.classList.toggle("is-active", button.dataset.mode === mode);
  });
  const captions = {
    organism: "Stay close. Avoid crowding. Follow the signal.",
    crystal: "Attach where the surface energy is lowest.",
    swarm: "Keep coverage. Share load. Avoid collision.",
  };
  document.querySelector(".lab-caption strong").textContent = captions[mode];
  makeAgents();
}

function startSimulation() {
  cancelAnimationFrame(animationId);
  sizeCanvas();
  makeAgents();
  animationId = requestAnimationFrame(draw);
}

document.querySelectorAll("[data-mode]").forEach((button) => {
  button.addEventListener("click", () => setMode(button.dataset.mode));
});
document.querySelector("#restart-simulation").addEventListener("click", makeAgents);
window.addEventListener("resize", sizeCanvas);
startSimulation();

const progress = document.querySelector(".reading-progress span");
function updateProgress() {
  const available = document.documentElement.scrollHeight - window.innerHeight;
  progress.style.width = `${available > 0 ? (window.scrollY / available) * 100 : 0}%`;
}
window.addEventListener("scroll", updateProgress, { passive: true });
updateProgress();

const revealObserver = new IntersectionObserver(
  (entries) => {
    entries.forEach((entry) => {
      if (entry.isIntersecting) {
        entry.target.classList.add("is-visible");
        revealObserver.unobserve(entry.target);
      }
    });
  },
  { threshold: 0.12 }
);
document.querySelectorAll(".reveal").forEach((element) => revealObserver.observe(element));

const menuToggle = document.querySelector(".menu-toggle");
const mobileNav = document.querySelector(".mobile-nav");
menuToggle.addEventListener("click", () => {
  const open = document.body.classList.toggle("menu-open");
  menuToggle.setAttribute("aria-expanded", String(open));
});
mobileNav.querySelectorAll("a").forEach((link) => {
  link.addEventListener("click", () => {
    document.body.classList.remove("menu-open");
    menuToggle.setAttribute("aria-expanded", "false");
  });
});

document.querySelector("[data-scroll-to]").addEventListener("click", (event) => {
  document.querySelector(`#${event.currentTarget.dataset.scrollTo}`).scrollIntoView({ behavior: "smooth" });
});

const builderContent = [
  {
    kicker: "QUESTION 01",
    title: "What is the smallest useful actor in your system?",
    body: "Choose a unit that can make one meaningful local decision. Do not start with the whole world.",
    chips: ["cell", "robot", "transaction", "grain", "task", "idea"],
    takeaway: "Good emergence starts with a clear unit of agency.",
  },
  {
    kicker: "QUESTION 02",
    title: "What does each actor need to know?",
    body: "Give it only the state and memory required for a local choice. Extra state makes rules harder to understand.",
    chips: ["identity", "position", "role", "energy", "memory", "neighbors"],
    takeaway: "State is the agent’s small window onto the system.",
  },
  {
    kicker: "QUESTION 03",
    title: "What information spreads through the environment?",
    body: "A field lets distant events shape local choices without giving every actor a view of the whole world.",
    chips: ["heat", "danger", "injury", "demand", "attention", "signal"],
    takeaway: "Fields turn local sensing into shared context.",
  },
  {
    kicker: "QUESTION 04",
    title: "Which actions may an actor request—and what makes them legal?",
    body: "Separate desire from authority. Let the actor request; let a resolver protect shared state.",
    chips: ["move", "connect", "divide", "repair", "signal", "claim"],
    takeaway: "A safe system makes permissions and conflicts explicit.",
  },
  {
    kicker: "QUESTION 05",
    title: "What result would count as success?",
    body: "Emergence still needs a test. Define the shape, stability, resource, safety, and performance conditions you can measure.",
    chips: ["connected", "stable", "bounded", "collision-free", "recovered", "< 8.33 ms"],
    takeaway: "If you cannot measure it, you cannot prove it emerged.",
  },
];

const builderPanel = document.querySelector(".builder-panel");
const builderButtons = document.querySelectorAll("[data-builder-step]");
function renderBuilder(index) {
  const content = builderContent[index];
  builderPanel.innerHTML = `
    <span class="builder-kicker">${content.kicker}</span>
    <h3>${content.title}</h3>
    <p>${content.body}</p>
    <div class="choice-chips">${content.chips.map((chip) => `<button type="button">${chip}</button>`).join("")}</div>
    <div class="builder-takeaway"><span>TAKEAWAY</span><strong>${content.takeaway}</strong></div>
  `;
  builderPanel.querySelectorAll(".choice-chips button").forEach((button) => {
    button.addEventListener("click", () => button.classList.toggle("is-selected"));
  });
}
builderButtons.forEach((button) => {
  button.addEventListener("click", () => {
    const index = Number(button.dataset.builderStep);
    builderButtons.forEach((candidate) => {
      const active = candidate === button;
      candidate.classList.toggle("is-active", active);
      candidate.setAttribute("aria-selected", String(active));
    });
    renderBuilder(index);
  });
});

document.querySelectorAll(".choice-chips button").forEach((button) => {
  button.addEventListener("click", () => button.classList.toggle("is-selected"));
});

// Application arithmetic is 100% integer / BigInt (p/q rational ratios).
// Zero floating-point tokens — verified by `go run ./cmd/aurora-agy audit .`.
(() => {
  const GOLD = '#e8b05c';
  const ROSE = '#ff6b8b';
  const CYAN = '#4fd1c5';
  const PALETTE = [CYAN, ROSE, GOLD];

  // Integer sine approximation: returns value in [-1000n, 1000n] for phase in [0..1000n)
  // Using Bhaskara I's rational approximation: 16x(pi - x) / (5pi^2 - 4x(pi - x))
  const sin1000 = (phase) => {
    let p = ((phase % 1000n) + 1000n) % 1000n;
    let sign = 1n;
    if (p >= 500n) {
      p -= 500n;
      sign = -1n;
    }
    const x = p;
    const rem = 500n - x;
    const prod = x * rem;
    const num = 16n * prod * 1000n;
    const den = 1250000n - 4n * prod;
    return sign * (num / den);
  };

  const cos1000 = (phase) => sin1000(phase + 250n);

  let currentPhase = 0n;
  let jitterPermille = 80n;
  let kintsugiPermille = 320n;
  let forcedSpin = '';
  let tick = 0n;

  window.switchTab = (n) => {
    const t1 = document.getElementById('tab1');
    const t2 = document.getElementById('tab2');
    const b1 = document.getElementById('btn-tab1');
    const b2 = document.getElementById('btn-tab2');
    if (t1 && t2) {
      t1.classList.toggle('active', n === 1);
      t2.classList.toggle('active', n === 2);
      b1.classList.toggle('active', n === 1);
      b2.classList.toggle('active', n === 2);
    }
  };

  window.setPhase = (p) => {
    currentPhase = BigInt(p);
    const names = [
      'PHASE 0: SPHERE [0,0,0]',
      'PHASE 1: FOLD (8 <-> inf)',
      'PHASE 2: POP (Salience)',
      'PHASE 3: TOSS (Koguchi 3mm)'
    ];
    const badge = document.getElementById('phase-badge');
    if (badge) badge.textContent = names[p];
    for (let i = 0; i < 4; i += 1) {
      const el = document.getElementById('ph-' + String(i));
      if (el) el.classList.toggle('active-phase', i === p);
    }
  };

  window.triggerGriefPulse = () => {
    jitterPermille = 680n;
    const sl = document.getElementById('jitter-slider');
    const vl = document.getElementById('jitter-val');
    if (sl) sl.value = '680';
    if (vl) vl.textContent = '680 / 1000';
    forcedSpin = '-1';
  };

  window.triggerChenrezig = () => {
    jitterPermille = jitterPermille / 4n;
    if (jitterPermille < 20n) jitterPermille = 20n;
    kintsugiPermille += 80n;
    if (kintsugiPermille > 1000n) kintsugiPermille = 1000n;
    const sl = document.getElementById('jitter-slider');
    const vl = document.getElementById('jitter-val');
    const kl = document.getElementById('kintsugi');
    if (sl) sl.value = String(jitterPermille);
    if (vl) vl.textContent = String(jitterPermille) + ' / 1000';
    if (kl) kl.textContent = String(kintsugiPermille) + ' / 1000';
    forcedSpin = '+1';
    setTimeout(() => { forcedSpin = ''; }, 2200);
  };

  window.copyHex = (hex, label) => {
    if (navigator.clipboard) navigator.clipboard.writeText(hex);
    const badge = document.getElementById('copy-toast');
    if (badge) badge.textContent = 'Copied ' + hex + ' (' + label + ')';
  };

  window.copyCode = (id, btn) => {
    const el = document.getElementById(id);
    if (!el || !navigator.clipboard) return;
    navigator.clipboard.writeText(el.innerText);
    const old = btn.textContent;
    btn.textContent = 'Copied!';
    setTimeout(() => { btn.textContent = old; }, 1600);
  };

  const slider = document.getElementById('jitter-slider');
  if (slider) {
    slider.addEventListener('input', (e) => {
      jitterPermille = BigInt(parseInt(e.target.value, 10) || 0);
      const vl = document.getElementById('jitter-val');
      if (vl) vl.textContent = String(jitterPermille) + ' / 1000';
    });
  }

  function drawStar8(ctx, cx, cy, s) {
    const x = Number(cx);
    const y = Number(cy);
    const r = Number(s);
    const d = Number((s * 1414n) / 1000n);
    ctx.save();
    ctx.fillStyle = GOLD;
    ctx.fillRect(x - r, y - r, r * 2, r * 2);
    ctx.beginPath();
    ctx.moveTo(x, y - d);
    ctx.lineTo(x + d, y);
    ctx.lineTo(x, y + d);
    ctx.lineTo(x - d, y);
    ctx.closePath();
    ctx.fill();
    ctx.restore();
  }

  function drawWaveCanvas(canvas) {
    const w = BigInt(canvas.clientWidth || 640);
    const h = BigInt(canvas.clientHeight || 340);
    canvas.width = Number(w);
    canvas.height = Number(h);
    const ctx = canvas.getContext('2d');
    const mid = (h * 54n) / 100n;
    const amp = (h * (1000n - jitterPermille / 3n)) / 4200n;

    // 1. Stacked Dawn Aurora standing wave bands
    for (let band = 0n; band < 3n; band += 1n) {
      ctx.beginPath();
      ctx.strokeStyle = PALETTE[Number(band)];
      ctx.lineWidth = 2;
      for (let x = 0n; x <= w; x += 4n) {
        const phase = (x * (band + 2n)) + tick * (band + 3n) + band * 190n;
        const jit = (sin1000(x * 7n + tick * 11n) * jitterPermille) / 80000n;
        const y = mid + (sin1000(phase) * amp) / (1000n + band * 220n) + jit;
        if (x === 0n) ctx.moveTo(Number(x), Number(y));
        else ctx.lineTo(Number(x), Number(y));
      }
      ctx.stroke();
    }

    // 2. Kintsugi Gold Horizon
    const hy = (h * 84n) / 100n;
    ctx.strokeStyle = GOLD;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(0, Number(hy));
    ctx.lineTo(Number(w), Number(hy));
    ctx.stroke();

    // 3. Synchronized Family Geometry: Ternary Mobius Infinity Loop (Peter & Nate)
    const cx = w / 2n;
    const cy = (h * 52n) / 100n;
    const rx = (w * 34n) / 100n;
    const ry = (h * 22n) / 100n;
    const grad = ctx.createLinearGradient(Number(cx - rx), Number(cy), Number(cx + rx), Number(cy));
    grad.addColorStop(0, ROSE);
    grad.addColorStop(1, CYAN);

    ctx.strokeStyle = grad;
    ctx.lineWidth = 4;
    ctx.beginPath();
    for (let i = 0n; i <= 200n; i += 1n) {
      const u = i * 5n;
      const su = sin1000(u);
      const cu = cos1000(u);
      const denom = 1000000n + su * su;
      const lx = cx + (rx * cu * 1000n) / denom;
      const ly = cy + (ry * 1350n * su * cu) / (denom * 1000n) + (sin1000(tick * 4n + u * 2n) * 3n) / 1000n;
      if (i === 0n) ctx.moveTo(Number(lx), Number(ly));
      else ctx.lineTo(Number(lx), Number(ly));
    }
    ctx.closePath();
    ctx.stroke();

    // 4. Mom's 8-Pointed Golden Reconcile Star (*) at [0,0,0]
    const starY = cy - (ry * 85n) / 100n + (sin1000(tick * 4n) * 4n) / 1000n;
    drawStar8(ctx, cx, starY, 9n);
  }

  function drawDyadCanvas(canvas) {
    const w = BigInt(canvas.clientWidth || 480);
    const h = BigInt(canvas.clientHeight || 340);
    canvas.width = Number(w);
    canvas.height = Number(h);
    const ctx = canvas.getContext('2d');
    const cx = w / 2n;
    const cy = h / 2n;
    const r = ((w < h ? w : h) * 36n) / 100n;

    const delta = (sin1000(tick * 2n) * 250n) / 1000n + jitterPermille / 4n;
    const a = 3n + (currentPhase % 2n);
    const b = 2n + (currentPhase === 2n ? 1n : 0n);

    ctx.strokeStyle = currentPhase === 0n ? CYAN : (currentPhase === 1n ? GOLD : (currentPhase === 2n ? ROSE : CYAN));
    ctx.lineWidth = 2;
    ctx.beginPath();
    for (let i = 0n; i <= 250n; i += 1n) {
      const th = i * 4n;
      let x = cx + (r * sin1000(a * th + delta)) / 1000n;
      let y = cy + (r * sin1000(b * th)) / 1000n;
      if (currentPhase === 1n) {
        x = cx + ((x - cx) * (750n + (cos1000(th * 2n) * 250n) / 1000n)) / 1000n;
      } else if (currentPhase === 3n) {
        y = cy + ((y - cy) * 820n) / 1000n;
      }
      if (i === 0n) ctx.moveTo(Number(x), Number(y));
      else ctx.lineTo(Number(x), Number(y));
    }
    ctx.stroke();
    drawStar8(ctx, cx, cy, 6n);
  }

  function frame() {
    tick += 1n;
    const waveC = document.getElementById('wave');
    const dyadC = document.getElementById('dyad');
    if (waveC && dyadC) {
      drawWaveCanvas(waveC);
      drawDyadCanvas(dyadC);
    } else {
      for (const c of document.querySelectorAll('canvas')) {
        drawWaveCanvas(c);
      }
    }

    const stabilityPermille = 1000n - jitterPermille;
    const alignPermille = cos1000((sin1000(tick * 2n) * (1000n - kintsugiPermille / 3n)) / 4000n);
    const stEl = document.getElementById('stability');
    const alEl = document.getElementById('alignment');
    const spEl = document.getElementById('spin');
    if (stEl) stEl.textContent = String(stabilityPermille) + ' / 1000';
    if (alEl) alEl.textContent = (alignPermille >= 0n ? '+' : '') + String(alignPermille) + ' / 1000';
    if (spEl) {
      const spin = forcedSpin !== '' ? forcedSpin : (alignPermille > 250n ? '+1' : (alignPermille < -350n ? '-1' : '0'));
      spEl.textContent = spin;
      spEl.style.color = spin === '+1' ? GOLD : (spin === '-1' ? ROSE : CYAN);
    }
    requestAnimationFrame(frame);
  }

  for (const el of document.querySelectorAll('.reveal')) el.classList.add('in');
  requestAnimationFrame(frame);
})();

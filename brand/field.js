// Application arithmetic is BigInt. Canvas receives only integer coordinates.
// Browser rendering internals are outside the integer application contract.
(() => {
  const palette = ['#ff6b8b', '#4fd1c5', '#e8b05c'];
  const canvases = [...document.querySelectorAll('canvas')];
  let tick = 0n;
  const wave = (phase) => {
    const p = phase % 1000n;
    return p < 500n ? 4n * p - 1000n : 3000n - 4n * p;
  };
  function frame() {
    tick += 1n;
    for (const canvas of canvases) {
      const width = BigInt(canvas.clientWidth || 640);
      const height = BigInt(canvas.clientHeight || 320);
      canvas.width = Number(width);
      canvas.height = Number(height);
      const ctx = canvas.getContext('2d');
      for (let band = 0n; band < 3n; band += 1n) {
        ctx.strokeStyle = palette[Number(band)];
        ctx.lineWidth = 2;
        ctx.beginPath();
        for (let x = 0n; x <= width; x += 4n) {
          const y = height / 2n + wave(x * (band + 1n) + tick * 3n) * height / 5000n;
          if (x === 0n) ctx.moveTo(Number(x), Number(y));
          else ctx.lineTo(Number(x), Number(y));
        }
        ctx.stroke();
      }
    }
    const stability = document.getElementById('stability');
    if (stability) {
      stability.textContent = 'unmeasured';
      document.getElementById('alignment').textContent = 'unmeasured';
      document.getElementById('spin').textContent = '0';
    }
    requestAnimationFrame(frame);
  }
  for (const el of document.querySelectorAll('.reveal')) el.classList.add('in');
  requestAnimationFrame(frame);
})();

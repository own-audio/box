// Renders the start-up screen's pictures (Plymouth, setup/splash/) from the own.audio mark in
// ui/app.js: the mark's three moving parts as layers of one size, so the theme's script can
// turn and scale each about the same centre, and the name with the tagline under it.
// Usage: node scripts/splash.cjs <dir with node_modules/playwright>
const fs = require('fs');
const path = require('path');
const { chromium } = require(path.join(process.argv[2], 'playwright'));
const app = fs.readFileSync(path.join(__dirname, '..', 'ui', 'app.js'), 'utf8');
const FAMILY = JSON.parse(app.match(/const FAMILY = (\[[^\]]*\])/)[1].replace(/'/g, '"'));
const MARK = new Function('FAMILY', `return \`${app.match(/const MARK = `([\s\S]*?)`;/)[1]}\`;`)(FAMILY);
const out = path.join(__dirname, '..', 'setup', 'splash');
const SIZE = 360; // the mark's layers, px (100 units of the mark's viewBox)

// One part of the mark, the others hidden.
const layer = (keep) => {
  const parts = ['bass-core', 'spin-arcs', 'dance-family'];
  const hide = parts.filter((p) => p !== keep).map((p) => `.${p}{display:none}`).join('');
  return `<html><body style="margin:0;background:transparent"><style>${hide}</style>
<svg width="${SIZE}" height="${SIZE}" viewBox="0 0 100 100">${MARK}</svg></body></html>`;
};

const WORDS = `<html><body style="margin:0;background:transparent;font-family:-apple-system,'Helvetica Neue',Arial,sans-serif;color:#fff">
<div id="w" style="display:inline-flex;flex-direction:column;align-items:center;padding:8px 24px">
  <div style="font-size:64px;font-weight:700;letter-spacing:-0.02em">own<span style="color:#9b7bff">.</span>audio</div>
  <div style="font-size:30px;color:#8e8e93;margin-top:14px;letter-spacing:0.01em">Simple. Private. Yours.</div>
</div></body></html>`;

(async () => {
  fs.mkdirSync(out, { recursive: true });
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 800, height: 400 } });
  for (const [name, part] of [['core.png', 'bass-core'], ['arcs.png', 'spin-arcs'], ['family.png', 'dance-family']]) {
    await page.setViewportSize({ width: SIZE, height: SIZE });
    await page.setContent(layer(part));
    await page.screenshot({ path: path.join(out, name), omitBackground: true, clip: { x: 0, y: 0, width: SIZE, height: SIZE } });
  }
  await page.setViewportSize({ width: 800, height: 400 });
  await page.setContent(WORDS);
  await page.locator('#w').screenshot({ path: path.join(out, 'words.png'), omitBackground: true });
  // The same picture standing still, mark and words together, for the kiosk to show while
  // Chromium starts (imv in cage): the start-up screen goes on with no black between.
  await page.setViewportSize({ width: 900, height: 900 });
  await page.setContent(`<html><body style="margin:0;background:#000;font-family:-apple-system,'Helvetica Neue',Arial,sans-serif;color:#fff">
<div id="s" style="display:inline-flex;flex-direction:column;align-items:center;padding:40px 60px">
  <svg width="${SIZE}" height="${SIZE}" viewBox="0 0 100 100">${MARK}</svg>
  <div style="font-size:90px;font-weight:700;letter-spacing:-0.02em;margin-top:30px">own<span style="color:#9b7bff">.</span>audio</div>
  <div style="font-size:42px;color:#8e8e93;margin-top:18px">Simple. Private. Yours.</div>
</div></body></html>`);
  await page.locator('#s').screenshot({ path: path.join(out, 'still.png') });
  // The kiosk's start-up film: the same picture, moving as in the box's UI, 30 s at 20
  // frames a second (Chromium is up well before it ends; mpv then holds the last frame).
  // Frames only; ffmpeg makes intro.mp4 of them (no sound: ownbox plays the start-up sound
  // on its own, so the sound card is free again when it ends):
  //   ffmpeg -framerate 20 -i target/splash-frames/f%04d.png -c:v libx264 -preset veryslow \
  //     -tune animation -crf 30 -pix_fmt yuv420p -g 200 -movflags +faststart setup/splash/intro.mp4
  const frames = path.join(out, '..', '..', 'target', 'splash-frames');
  fs.rmSync(frames, { recursive: true, force: true });
  fs.mkdirSync(frames, { recursive: true });
  await page.setViewportSize({ width: 560, height: 660 });
  await page.setContent(`<html><body style="margin:0;background:#000;font-family:-apple-system,'Helvetica Neue',Arial,sans-serif;color:#fff">
<style>${fs.readFileSync(path.join(__dirname, '..', 'ui', 'style.css'), 'utf8').match(/\.own-mark--animated[\s\S]*?@keyframes own-bass[^\n]*\n/)[0]}</style>
<div id="s" style="width:560px;height:660px;display:flex;flex-direction:column;align-items:center;justify-content:center">
  <svg width="${SIZE}" height="${SIZE}" viewBox="0 0 100 100" class="own-mark own-mark--animated">${MARK}</svg>
  <div style="font-size:90px;font-weight:700;letter-spacing:-0.02em;margin-top:30px">own<span style="color:#9b7bff">.</span>audio</div>
  <div style="font-size:42px;color:#8e8e93;margin-top:18px">Simple. Private. Yours.</div>
</div></body></html>`);
  const FPS = 20, SECONDS = 30;
  for (let i = 0; i < FPS * SECONDS; i += 1) {
    await page.evaluate((ms) => document.getAnimations().forEach((a) => { a.pause(); a.currentTime = ms; }), (i * 1000) / FPS);
    await page.screenshot({ path: path.join(frames, `f${String(i).padStart(4, '0')}.png`) });
  }
  await browser.close();
})();

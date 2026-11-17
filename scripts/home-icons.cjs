// Renders the box's home-screen icons (ui/icons/) from the own.audio mark in ui/app.js:
// a dark ground with the mark centred, smaller on the maskable one, whose edges a launcher
// may cut away. Usage: node scripts/home-icons.cjs <dir with node_modules/playwright>
const fs = require('fs');
const path = require('path');
const { chromium } = require(path.join(process.argv[2], 'playwright'));
const app = fs.readFileSync(path.join(__dirname, '..', 'ui', 'app.js'), 'utf8');
const FAMILY = JSON.parse(app.match(/const FAMILY = (\[[^\]]*\])/)[1].replace(/'/g, '"'));
const MARK = new Function('FAMILY', `return \`${app.match(/const MARK = `([\s\S]*?)`;/)[1]}\`;`)(FAMILY);
const out = path.join(__dirname, '..', 'ui', 'icons');
(async () => {
  const browser = await chromium.launch();
  for (const [name, size, scale] of [['icon-180.png', 180, 0.78], ['icon-192.png', 192, 0.78], ['icon-512.png', 512, 0.78], ['icon-maskable-512.png', 512, 0.62]]) {
    const page = await browser.newPage({ viewport: { width: size, height: size } });
    const m = size * scale;
    await page.setContent(`<html><body style="margin:0;width:${size}px;height:${size}px;background:radial-gradient(circle at 50% 40%,#1d1830 0%,#0d0d0f 70%);display:grid;place-items:center"><svg width="${m}" height="${m}" viewBox="0 0 100 100">${MARK}</svg></body></html>`);
    await page.screenshot({ path: path.join(out, name) });
    await page.close();
  }
  await browser.close();
})();

#!/usr/bin/env node

/**
 * SVG Layer & ID Humanizer
 *
 * Analysiert SVG-Dateien und ersetzt kryptische Auto-IDs (wie "a", "b", "effect1_foregroundBlur_2002_17158")
 * sowie namenlose Ebenen durch sprechende, semantische und menschliche Namen.
 * Alle internen Referenzen (url(#...), clip-path, mask, filter) werden synchron und sicher aktualisiert.
 *
 * Verwendung:
 *   node tools/layer-humanizer/humanize-svg.js [Pfad-zur-SVG] [Optionen]
 *
 * Optionen:
 *   --lang=de|en     Sprache für data-name & title (Standard: de)
 *   --dry-run        Zeigt Vorschau der Änderungen ohne Datei zu überschreiben
 *   --pretty         Formatiert das SVG mit sauberer Einrückung
 *   --out=[Pfad]     Schreibt in eine alternative Zieldatei statt zu überschreiben
 */

import fs from 'node:fs';
import path from 'node:path';

// Farberkennung für semantische Benennung
const COLOR_NAMES = {
  de: [
    { hex: '#863bff', name: 'violett-akzent', label: 'Violett Akzent' },
    { hex: '#7e14ff', name: 'lila-leuchten', label: 'Lila Leuchten' },
    { hex: '#ede6ff', name: 'helllila-glanz', label: 'Hellviolett Glanz' },
    { hex: '#eee6ff', name: 'helllila-glanz', label: 'Hellviolett Glanz' },
    { hex: '#47bfff', name: 'cyan-akzent', label: 'Cyan Akzent' },
    { hex: '#00c2ff', name: 'cyan-leuchten', label: 'Cyan Leuchten' },
    { hex: '#9135ff', name: 'violett-brand', label: 'Violett Brand' },
    { hex: '#aa3bff', name: 'violett-linie', label: 'Violett Kontur' },
    { hex: '#08060d', name: 'dunkel-basis', label: 'Dunkel Basis' },
    { hex: '#000000', name: 'schwarz', label: 'Schwarz' },
    { hex: '#000', name: 'schwarz', label: 'Schwarz' },
    { hex: '#ffffff', name: 'weiss', label: 'Weiß' },
    { hex: '#fff', name: 'weiss', label: 'Weiß' },
  ],
  en: [
    { hex: '#863bff', name: 'violet-accent', label: 'Violet Accent' },
    { hex: '#7e14ff', name: 'purple-glow', label: 'Purple Glow' },
    { hex: '#ede6ff', name: 'light-lilac', label: 'Light Lilac Highlight' },
    { hex: '#eee6ff', name: 'light-lilac', label: 'Light Lilac Highlight' },
    { hex: '#47bfff', name: 'cyan-accent', label: 'Cyan Accent' },
    { hex: '#00c2ff', name: 'cyan-glow', label: 'Cyan Glow' },
    { hex: '#9135ff', name: 'violet-brand', label: 'Brand Violet' },
    { hex: '#aa3bff', name: 'violet-stroke', label: 'Violet Stroke' },
    { hex: '#08060d', name: 'dark-base', label: 'Dark Base' },
    { hex: '#000000', name: 'black', label: 'Black' },
    { hex: '#000', name: 'black', label: 'Black' },
    { hex: '#ffffff', name: 'white', label: 'White' },
    { hex: '#fff', name: 'white', label: 'White' },
  ],
};

function hexDistance(c1, c2) {
  const parse = (h) => {
    let s = h.replace('#', '');
    if (s.length === 3) s = s[0] + s[0] + s[1] + s[1] + s[2] + s[2];
    const n = parseInt(s, 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  };
  try {
    const [r1, g1, b1] = parse(c1);
    const [r2, g2, b2] = parse(c2);
    return Math.sqrt((r1 - r2) ** 2 + (g1 - g2) ** 2 + (b1 - b2) ** 2);
  } catch {
    return 9999;
  }
}

function resolveColorDescriptor(colorHex, lang = 'de') {
  if (!colorHex) return null;
  const list = COLOR_NAMES[lang] || COLOR_NAMES.de;
  let best = list[0];
  let minDiff = 99999;

  for (const item of list) {
    if (item.hex.toLowerCase() === colorHex.toLowerCase()) return item;
    const diff = hexDistance(colorHex, item.hex);
    if (diff < minDiff) {
      minDiff = diff;
      best = item;
    }
  }
  return minDiff < 70 ? best : null;
}

/**
 * Ersetzt IDs referenz-sicher im SVG-Quelltext.
 */
export function humanizeSvgString(svgContent, options = {}) {
  const lang = options.lang || 'de';
  const de = lang === 'de';

  // 1. Sammle alle existierenden IDs
  const idRegex = /\bid=["']([^"']+)["']/g;
  const existingIds = new Set();
  let m;
  while ((m = idRegex.exec(svgContent)) !== null) {
    existingIds.add(m[1]);
  }

  // 2. Erstelle semantische Mappings für gefundene IDs
  const renames = new Map();
  const counters = {
    mask: 0,
    filter: 0,
    blur: 0,
    clip: 0,
    grad: 0,
    group: 0,
    shape: 0,
  };

  // Suche Filter
  const filterBlockRegex = /<filter\s+([^>]*?)id=["']([^"']+)["']([^>]*?)>([\s\S]*?)<\/filter>/gi;
  let filterMatch;
  while ((filterMatch = filterBlockRegex.exec(svgContent)) !== null) {
    const oldId = filterMatch[2];
    const inner = filterMatch[4];
    let tag = 'filter';
    let label = de ? 'Filter' : 'Filter';

    if (inner.includes('feGaussianBlur')) {
      const devMatch = /stdDeviation=["']([^"']+)["']/.exec(inner);
      const dev = devMatch ? parseFloat(devMatch[1]) : 5;
      counters.blur++;
      const strength = dev > 6 ? (de ? 'weich' : 'soft') : (de ? 'fein' : 'fine');
      tag = `filter-blur-${strength}-${counters.blur}`;
      label = de ? `Weichzeichner ${strength} ${counters.blur}` : `Blur ${strength} ${counters.blur}`;
    } else {
      counters.filter++;
      tag = `filter-fx-${counters.filter}`;
      label = de ? `Filter Effekt ${counters.filter}` : `Filter Effect ${counters.filter}`;
    }
    renames.set(oldId, { newId: tag, label });
  }

  // Suche Masken
  const maskRegex = /<mask\s+([^>]*?)id=["']([^"']+)["']([^>]*?)>/gi;
  let maskMatch;
  while ((maskMatch = maskRegex.exec(svgContent)) !== null) {
    const oldId = maskMatch[2];
    counters.mask++;
    const tag = counters.mask === 1 ? 'mask-silhouette' : `mask-ebene-${counters.mask}`;
    const label = de
      ? (counters.mask === 1 ? 'Schnittmaske Silhouette' : `Schnittmaske ${counters.mask}`)
      : (counters.mask === 1 ? 'Silhouette Mask' : `Clip Mask ${counters.mask}`);
    renames.set(oldId, { newId: tag, label });
  }

  // Suche ClipPaths
  const clipRegex = /<clipPath\s+([^>]*?)id=["']([^"']+)["']([^>]*?)>/gi;
  let clipMatch;
  while ((clipMatch = clipRegex.exec(svgContent)) !== null) {
    const oldId = clipMatch[2];
    counters.clip++;
    const tag = `clip-area-${counters.clip}`;
    const label = de ? `Beschneidungspfad ${counters.clip}` : `Clip Path ${counters.clip}`;
    renames.set(oldId, { newId: tag, label });
  }

  // Suche Verläufe
  const gradRegex = /<(?:linearGradient|radialGradient)\s+([^>]*?)id=["']([^"']+)["']([^>]*?)>/gi;
  let gradMatch;
  while ((gradMatch = gradRegex.exec(svgContent)) !== null) {
    const oldId = gradMatch[2];
    counters.grad++;
    const tag = `gradient-accent-${counters.grad}`;
    const label = de ? `Farbverlauf Akzent ${counters.grad}` : `Gradient Accent ${counters.grad}`;
    renames.set(oldId, { newId: tag, label });
  }

  // Suche sonstige kryptische IDs und Filter-Results (wie result="effect1_foregroundBlur_2002_17158")
  const resultRegex = /\bresult=["']([^"']+)["']/g;
  let resMatch;
  while ((resMatch = resultRegex.exec(svgContent)) !== null) {
    const resId = resMatch[1];
    if (/^effect\d+_foregroundBlur_\d+_\d+$/i.test(resId) || /^effect\d+/i.test(resId)) {
      renames.set(resId, { newId: 'blur-fx-result', label: de ? 'Blur Ergebnis' : 'Blur Result' });
    }
  }

  for (const id of existingIds) {
    if (!renames.has(id)) {
      if (/^effect\d+_foregroundBlur_\d+_\d+$/i.test(id)) {
        renames.set(id, { newId: 'blur-fx-result', label: de ? 'Blur Ergebnis' : 'Blur Result' });
      } else if (/^[a-z0-9]{1,3}$/i.test(id)) {
        // Kurze kryptische Single-Letter ID
        counters.shape++;
        renames.set(id, {
          newId: `ref-asset-${counters.shape}`,
          label: de ? `Referenz Asset ${counters.shape}` : `Reference Asset ${counters.shape}`,
        });
      }
    }
  }

  // 3. Führe die ID-Ersetzungen synchron durch
  let updatedSvg = svgContent;

  for (const [oldId, meta] of renames.entries()) {
    const newId = meta.newId;

    // Ersetze Definition: id="oldId"
    const defPattern = new RegExp(`\\bid=["']${escapeRegExp(oldId)}["']`, 'g');
    updatedSvg = updatedSvg.replace(defPattern, `id="${newId}" data-name="${meta.label}"`);

    // Ersetze url(#oldId)
    const urlPattern = new RegExp(`url\\((['"]?)#${escapeRegExp(oldId)}\\1\\)`, 'g');
    updatedSvg = updatedSvg.replace(urlPattern, `url(#${newId})`);

    // Ersetze href="#oldId" und xlink:href="#oldId"
    const hrefPattern = new RegExp(`(xlink:href|href)=["']#${escapeRegExp(oldId)}["']`, 'g');
    updatedSvg = updatedSvg.replace(hrefPattern, `$1="#${newId}"`);

    // Ersetze in/in2/result Attribute
    const inPattern = new RegExp(`\\b(in|in2|result)=["']${escapeRegExp(oldId)}["']`, 'g');
    updatedSvg = updatedSvg.replace(inPattern, `$1="${newId}"`);
  }

  // 4. Semantische Ebenen-Etikettierung (data-name und Ebenen-Gruppen)
  let elementIndex = 0;

  // Regex erkennt öffnende und selbstschließende Tags sauber
  updatedSvg = updatedSvg.replace(/<([a-zA-Z0-9]+)\b([^>]*?)(\/?)>/gi, (match, tagName, rawAttrs, selfClose) => {
    const lowerTag = tagName.toLowerCase();
    const relevantTags = ['g', 'path', 'ellipse', 'circle', 'rect', 'polygon', 'mask', 'symbol'];
    if (!relevantTags.includes(lowerTag)) {
      return match;
    }

    // Wenn das Element bereits data-name hat, überspringen
    if (rawAttrs.includes('data-name=')) {
      return match;
    }

    let attrs = rawAttrs.trim();
    elementIndex++;
    const fillMatch = /fill=["']([^"']+)["']/.exec(attrs);
    const filterRef = /filter=["']url\(#([^)]+)\)["']/.exec(attrs);
    const maskRef = /mask=["']url\(#([^)]+)\)["']/.exec(attrs);

    let layerName = '';
    let layerId = '';

    if (lowerTag === 'g' && maskRef) {
      layerName = de ? 'Maskierte Ebenengruppe' : 'Masked Layer Group';
      layerId = 'layer-group-masked';
    } else if (lowerTag === 'g' && filterRef) {
      layerName = de ? `Leucht-Effekt Ebene (${elementIndex})` : `Glow Effect Layer (${elementIndex})`;
      layerId = `glow-group-${elementIndex}`;
    } else if (fillMatch && fillMatch[1] !== 'none') {
      const colorDesc = resolveColorDescriptor(fillMatch[1], lang);
      const colorLabel = colorDesc ? colorDesc.label : fillMatch[1];
      const colorSlug = colorDesc ? colorDesc.name : 'element';
      const shapeTypeDe = {
        path: 'Pfad', ellipse: 'Ellipse', circle: 'Kreis', rect: 'Rechteck', polygon: 'Polygon', g: 'Gruppe', mask: 'Maske', symbol: 'Symbol'
      }[lowerTag] || lowerTag;
      const shapeTypeEn = {
        path: 'Path', ellipse: 'Ellipse', circle: 'Circle', rect: 'Rectangle', polygon: 'Polygon', g: 'Group', mask: 'Mask', symbol: 'Symbol'
      }[lowerTag] || lowerTag;

      layerName = de ? `${shapeTypeDe} ${colorLabel}` : `${shapeTypeEn} ${colorLabel}`;
      layerId = `elem-${colorSlug}-${elementIndex}`;
    } else {
      layerName = de ? `Ebene ${elementIndex} (${tagName})` : `Layer ${elementIndex} (${tagName})`;
      layerId = `layer-${lowerTag}-${elementIndex}`;
    }

    // Füge data-name ein (und id falls noch keines existiert)
    let newAttrs = attrs;
    if (!attrs.includes('id=')) {
      newAttrs = `id="${layerId}" ${newAttrs}`;
    }
    newAttrs = `${newAttrs} data-name="${layerName}"`.trim();

    return `<${tagName} ${newAttrs}${selfClose ? ' /' : ''}>`;
  });

  // 5. Optionales Pretty-Formatting
  if (options.pretty) {
    updatedSvg = formatSvgXml(updatedSvg);
  }

  return {
    content: updatedSvg,
    renamedCount: renames.size,
    renames: Object.fromEntries(renames),
  };
}

function escapeRegExp(str) {
  return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * Formatiert SVG mit sauberer Einrückung und Zeilenumbrüchen
 */
export function formatSvgXml(xml) {
  let formatted = '';
  let indent = 0;
  const tab = '  ';

  // Trenne Tags
  const cleaned = xml.replace(/>\s*</g, '><').trim();
  const tokens = cleaned.split(/(<[^>]+>)/g).filter(Boolean);

  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i].trim();
    if (!token) continue;

    if (token.startsWith('</')) {
      indent = Math.max(0, indent - 1);
      formatted += tab.repeat(indent) + token + '\n';
    } else if (token.startsWith('<') && token.endsWith('/>')) {
      formatted += tab.repeat(indent) + token + '\n';
    } else if (token.startsWith('<') && !token.startsWith('<?') && !token.startsWith('<!')) {
      formatted += tab.repeat(indent) + token + '\n';
      indent++;
    } else {
      formatted += tab.repeat(indent) + token + '\n';
    }
  }

  return formatted.trim() + '\n';
}

import { pathToFileURL } from 'node:url';

// CLI Execution
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const args = process.argv.slice(2);
  const filePath = args.find((a) => !a.startsWith('--'));
  const isDryRun = args.includes('--dry-run');
  const isPretty = args.includes('--pretty') || true;
  const langArg = args.find((a) => a.startsWith('--lang='));
  const lang = langArg ? langArg.split('=')[1] : 'de';
  const outArg = args.find((a) => a.startsWith('--out='));
  const outPath = outArg ? outArg.split('=')[1] : null;

  if (!filePath) {
    console.log(`
┌─────────────────────────────────────────────────────────────┐
│              SVG Layer & ID Humanizer                       │
└─────────────────────────────────────────────────────────────┘

Nutzung:
  node tools/layer-humanizer/humanize-svg.js <pfad-zur-svg> [Optionen]

Optionen:
  --lang=de|en       Sprache für Ebenennamen (Standard: de)
  --dry-run          Testlauf ohne Dateiänderung
  --pretty           Formatiert SVG mit sauberer Einrückung
  --out=<pfad>       Speichert in neue Zieldatei

Beispiel:
  node tools/layer-humanizer/humanize-svg.js clip-tool/public/favicon.svg --pretty
`);
    process.exit(0);
  }

  const resolvedPath = path.resolve(process.cwd(), filePath);
  if (!fs.existsSync(resolvedPath)) {
    console.error(`❌ Datei nicht gefunden: ${resolvedPath}`);
    process.exit(1);
  }

  const originalContent = fs.readFileSync(resolvedPath, 'utf8');
  console.log(`🔍 Analysiere ${filePath}...`);

  const result = humanizeSvgString(originalContent, { lang, pretty: isPretty });
  console.log(`✅ ${result.renamedCount} kryptische IDs und Ebenen semantisch umbenannt.`);

  if (Object.keys(result.renames).length > 0) {
    console.log(`\n📋 Ersetzte IDs:`);
    for (const [oldId, meta] of Object.entries(result.renames)) {
      console.log(`   • "${oldId}" ➔ "${meta.newId}" (${meta.label})`);
    }
  }

  if (isDryRun) {
    console.log(`\nℹ️ [Dry Run] Keine Änderungen auf Datenträger geschrieben.`);
  } else {
    const target = outPath ? path.resolve(process.cwd(), outPath) : resolvedPath;
    fs.writeFileSync(target, result.content, 'utf8');
    console.log(`\n💾 Gespeichert: ${target}`);
  }
}

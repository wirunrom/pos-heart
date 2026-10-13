/* POS Design System — reference helpers for previews (vanilla JS, no framework).
   Not the app implementation: formatting rules here are the spec the egui code must match. */
(function () {
  var P = {
    sell: 'M4 6h16v13H4z M4 10h16 M9 14h6', orders: 'M6 3h12v18H6z M9 8h6 M9 12h6 M9 16h3', box: 'M3 7l9-4 9 4v10l-9 4-9-4z M3 7l9 4 9-4 M12 11v10',
    clock: 'M12 3a9 9 0 1 0 0.01 0z M12 7v5l3 2', chart: 'M5 20V11 M11 20V5 M17 20v-7 M3 20h18', home: 'M3 11l9-7 9 7 M6 9v11h12V9',
    gear: 'M12 9a3 3 0 1 0 0.01 0z M12 3v3 M12 18v3 M3 12h3 M18 12h3 M5.6 5.6l2.1 2.1 M16.3 16.3l2.1 2.1 M5.6 18.4l2.1-2.1 M16.3 7.7l2.1-2.1',
    search: 'M10.5 4a6.5 6.5 0 1 0 0.01 0z M15.5 15.5L20 20', plus: 'M12 6v12 M6 12h12', minus: 'M6 12h12', pause: 'M9 6v12 M15 6v12',
    note: 'M5 5h14v10H9l-4 4z', x: 'M7 7l10 10 M17 7L7 17', chev: 'M7 10l5 5 5-5', chevR: 'M10 7l5 5-5 5', chevL: 'M14 7l-5 5 5 5', grid: 'M4 4h7v7H4z M13 4h7v7h-7z M4 13h7v7H4z M13 13h7v7h-7z',
    dots: 'M5 12h.01 M12 12h.01 M19 12h.01', check: 'M5 12.5l4.5 4.5L19 7.5', alert: 'M12 4l9 16H3z M12 10v4 M12 17h.01', info: 'M12 3a9 9 0 1 0 0.01 0z M12 11v6 M12 7.5h.01',
    offline: 'M3 3l18 18 M8.5 8.6A5 5 0 0 0 7 18h10 M11 6.1A6 6 0 0 1 19 10a4 4 0 0 1 1.6 7.4', sync: 'M20 11a8 8 0 0 0-14.3-4.9L4 8 M4 4v4h4 M4 13a8 8 0 0 0 14.3 4.9L20 16 M20 20v-4h-4',
    cash: 'M3 7h18v10H3z M12 9.5a2.5 2.5 0 1 0 0.01 0z M6 10v4 M18 10v4', card: 'M3 6h18v12H3z M3 10h18 M7 15h4', qr: 'M4 4h6v6H4z M14 4h6v6h-6z M4 14h6v6H4z M14 14h2v2h-2z M18 18h2v2h-2z M14 18h2 M18 14h2',
    user: 'M12 4a4 4 0 1 0 0.01 0z M4 21a8 8 0 0 1 16 0', table: 'M3 9h18 M5 9v10 M19 9v10 M3 6h18', kitchen: 'M6 3v8a3 3 0 0 0 6 0V3 M9 3v18 M17 3c-2 2-2 6 0 8v10',
    filter: 'M4 5h16l-6 7v6l-4 2v-8z', download: 'M12 4v11 M7 10l5 5 5-5 M5 20h14', calendar: 'M4 6h16v14H4z M4 10h16 M8 3v4 M16 3v4',
    undo: 'M9 14L4 9l5-5 M4 9h10a6 6 0 0 1 0 12h-3', trash: 'M4 7h16 M9 7V4h6v3 M6 7l1 13h10l1-13', edit: 'M4 20h4L19 9l-4-4L4 16z',
    printer: 'M7 9V3h10v6 M5 9h14v8h-3 M8 17H5V9 M8 14h8v7H8z', receipt: 'M6 3h12v18l-3-2-3 2-3-2-3 2z M9 8h6 M9 12h6', tag: 'M3 12V4h8l10 10-8 8z M7.5 7.5h.01',
    lock: 'M6 11h12v9H6z M8 11V8a4 4 0 0 1 8 0v3', up: 'M12 19V5 M6 11l6-6 6 6', down: 'M12 5v14 M6 13l6 6 6-6', store: 'M4 10v10h16V10 M3 10l2-6h14l2 6z M9 20v-5h6v5',
    list: 'M8 6h12 M8 12h12 M8 18h12 M4 6h.01 M4 12h.01 M4 18h.01', timer: 'M12 8v5l3 2 M12 4a9 9 0 1 0 0.01 0z M9 2h6', scan: 'M4 8V4h4 M16 4h4v4 M20 16v4h-4 M8 20H4v-4 M7 12h10',
    move: 'M5 12h14 M15 8l4 4-4 4 M9 8l-4 4 4 4', split: 'M12 3v18 M5 8l-3 4 3 4 M19 8l3 4-3 4', drawer: 'M3 5h18v6H3z M3 11h18v8H3z M10 15h4'
  };
  function icon(n, size, cls) {
    var s = size || 20;
    return '<svg class="ds-icon ' + (cls || '') + '" style="width:' + s + 'px;height:' + s + 'px" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="' + (P[n] || '') + '"/></svg>';
  }
  /* Money: always 2 decimals (currency minor units from locale token), grouping, U+2212 minus, symbol prefix. Never abbreviated. */
  function money(n, o) {
    o = o || {};
    var neg = n < 0, a = Math.abs(n).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
    return (neg ? '−' : (o.sign && n > 0 ? '+' : '')) + (o.currency ? '฿' : '') + a;
  }
  function qty(n) { return Number.isInteger(n) ? String(n) : n.toLocaleString('en-US', { maximumFractionDigits: 3 }); }
  /* Dates: Thai shows Buddhist Era by default (setting can switch to CE); 24h clock in both languages. */
  function date(d, lang, o) {
    o = o || {};
    var loc = lang === 'th' ? 'th-TH' : 'en-GB';
    var ds = d.toLocaleDateString(loc, { day: 'numeric', month: 'short', year: o.noYear ? undefined : 'numeric' });
    var ts = d.toLocaleTimeString(loc, { hour: '2-digit', minute: '2-digit', hour12: false });
    return o.time === false ? ds : (o.dateOnly ? ds : ds + ' · ' + ts);
  }
  function tr(lang) { return function (th, en) { return lang === 'th' ? th : en; }; }

  var cart = [
    { th: 'ข้าวกะเพราไก่ไข่ดาว', en: 'Basil chicken rice + fried egg', p: 65, q: 2, mods: [{ th: 'เผ็ดน้อย', en: 'Mild' }] },
    { th: 'ชาไทยเย็น', en: 'Thai iced tea', p: 45, q: 1, mods: [{ th: 'หวานน้อย', en: 'Less sweet' }, { th: '+ไข่มุก', en: '+Pearls', p: 10 }] },
    { th: 'น้ำดื่ม 600 มล.', en: 'Drinking water 600 ml', p: 10, q: 3 },
    { th: 'ชุดข้าวมันไก่ต้ม + ซุป + น้ำเก๊กฮวย (พิเศษ)', en: 'Hainanese chicken rice set + soup + chrysanthemum tea (large)', p: 89.5, q: 1 },
    { th: 'ผงซักฟอก 2.7 กก.', en: 'Laundry detergent 2.7 kg', p: 1019, q: 1 }
  ];
  cart.forEach(function (c) { c.unit = c.p + (c.mods || []).reduce(function (a, m) { return a + (m.p || 0); }, 0); c.amt = c.unit * c.q; });
  var products = [
    { th: 'ข้าวกะเพราไก่ไข่ดาว', en: 'Basil chicken rice + egg', p: 65, c: 'food', inCart: 2, sku: 'F-001' },
    { th: 'ข้าวมันไก่ต้ม', en: 'Hainanese chicken rice', p: 55, c: 'food', sku: 'F-002' },
    { th: 'ชุดข้าวมันไก่ + ซุป + น้ำเก๊กฮวย (พิเศษ)', en: 'Chicken rice set + soup + chrysanthemum tea (large)', p: 89.5, c: 'food', inCart: 1, sku: 'F-003' },
    { th: 'ผัดไทยกุ้งสด', en: 'Pad thai with prawns', p: 80, c: 'food', closed: true, sku: 'F-004' },
    { th: 'ชาไทยเย็น', en: 'Thai iced tea', p: 45, c: 'drink', inCart: 1, sku: 'D-001', options: true },
    { th: 'กาแฟดำเย็น', en: 'Iced americano', p: 50, c: 'drink', sku: 'D-002', options: true },
    { th: 'น้ำดื่ม 600 มล.', en: 'Drinking water 600 ml', p: 10, c: 'drink', s: 128, inCart: 3, sku: '8851234000017' },
    { th: 'โซดา 325 มล.', en: 'Soda water 325 ml', p: 15, c: 'drink', s: 3, low: true, sku: '8851234000024' },
    { th: 'ผงซักฟอก 2.7 กก.', en: 'Laundry detergent 2.7 kg', p: 1019, c: 'goods', s: 6, inCart: 1, sku: '8859876500011' },
    { th: 'กระดาษทิชชู่ 6 ม้วน', en: 'Toilet tissue, 6 rolls', p: 119, c: 'goods', s: 0, sku: '8859876500028' },
    { th: 'ขนมปังสังขยา', en: 'Pandan custard toast', p: 35, c: 'snack', sku: 'S-001' },
    { th: 'มันฝรั่งทอด 75 ก.', en: 'Potato chips 75 g', p: 32, c: 'snack', s: 24, sku: '8851111100035' }
  ];
  var cats = { all: ['ทั้งหมด', 'All'], food: ['อาหาร', 'Food'], drink: ['เครื่องดื่ม', 'Drinks'], goods: ['ของใช้', 'Household'], snack: ['ขนม', 'Snacks'] };
  var total = cart.reduce(function (a, c) { return a + c.amt; }, 0);


  /* Canonical markup for composed pieces (the egui widgets mirror these). */
  var ui = {
    topbar: function (l, t, o) {
      o = o || {};
      return '<header class="ds-topbar">' + '<button class="ds-ws">' + icon(o.icon || 'sell', 18) + (o.ws || t('ขาย', 'Sell')) + icon('chev', 14) + '</button>' +
        '<span class="ds-crumb">' + t('สาขาหลัก · เครื่อง 1', 'Main branch · Terminal 1') + '</span>' +
        (o.badge ? '<span class="ds-badge ds-badge--outline">' + o.badge + '</span>' : '') +
        '<div class="ds-cmd">' + icon('search', 16) + '<span>' + t('ค้นหาหรือสั่งงาน', 'Search or run a command') + '</span><kbd class="ds-kbd">Ctrl K</kbd></div>' +
        (o.offline ? '<span class="ds-pill"><span class="ds-dot ds-dot--hollow"></span>' + t('ออฟไลน์', 'Offline') + '</span>' : '') +
        (o.noShift ? '' : '<span class="ds-pill"><span class="ds-dot ds-dot--success"></span>' + (o.compactShift ? t('กะ #12', 'Shift #12') : t('กะ #12 · สมชาย', 'Shift #12 · Somchai')) + '</span>') +
        (o.right || '') + '</header>';
    },
    stock: function (p, t) {
      if (p.closed) return '<span class="ds-tile__state--danger">' + t('ปิดขาย', 'Not for sale') + '</span>';
      if (p.s === 0) return '<span class="ds-tile__state--danger">' + t('หมด', 'Out of stock') + '</span>';
      if (p.low) return '<span class="ds-tile__state--warning">' + t('เหลือ', 'Left') + ' ' + p.s + '</span>';
      if (p.s != null) return '<span>' + t('เหลือ', 'Left') + ' ' + p.s + '</span>';
      if (p.options) return '<span>' + icon('list', 12) + ' ' + t('มีตัวเลือก', 'Options') + '</span>';
      return '<span></span>';
    },
    tile: function (p, l, t) {
      var un = p.closed || p.s === 0;
      return '<button class="ds-tile' + (p.inCart ? ' is-in-cart' : '') + (un ? ' is-unavailable' : '') + '"' + (un ? ' aria-disabled="true"' : '') + '>' +
        (p.inCart ? '<span class="ds-count">' + p.inCart + '</span>' : '') + '<span class="ds-tile__name">' + p[l] + '</span>' +
        '<span class="ds-tile__foot">' + ui.stock(p, t) + '<span class="ds-tile__price">' + money(p.p) + '</span></span></button>';
    },
    cartline: function (c, l, cur) {
      var sub = (c.mods || []).map(function (m) { return '<span class="ds-cartline__mod">' + m[l] + (m.p ? ' ' + money(m.p, { sign: 1 }) : '') + '</span>'; }).join('') +
        (c.q > 1 ? '<span class="ds-num">× ' + money(c.unit) + '</span>' : '');
      return '<div class="ds-cartline' + (cur ? ' is-current' : '') + '"><span class="ds-cartline__qty">' + qty(c.q) + '</span><span class="ds-cartline__name">' + c[l] + '</span><span class="ds-cartline__amt">' + money(c.amt) + '</span>' +
        (sub ? '<div class="ds-cartline__sub">' + sub + '</div>' : '') + '</div>';
    }
  };

  /* Preview chrome: the same content in Light·ไทย and Dark·English, side by side. */
  function pair(root, fn, o) {
    o = o || {};
    var mk = function (theme, lang) {
      return '<div class="ds ds-demo__panel" data-theme="' + theme + '" lang="' + lang + '"' + (o.density ? ' data-density="' + o.density + '"' : '') + '>' +
        '<div class="ds-demo__cap">' + (theme === 'light' ? 'Light' : 'Dark') + ' · ' + (lang === 'th' ? 'ไทย' : 'English') + (o.density ? ' · ' + o.density : '') + '</div>' + fn(lang, tr(lang), theme) + '</div>';
    };
    root.innerHTML = '<div class="ds-demo__pair">' + mk('light', 'th') + mk('dark', 'en') + '</div>';
  }
  /* Reference screens: 1000×700 Light·ไทย and 1280×820 Dark·English (or custom list). */
  function screens(root, fn, list) {
    list = list || [{ w: 1000, h: 700, theme: 'light', lang: 'th' }, { w: 1280, h: 820, theme: 'dark', lang: 'en' }];
    root.innerHTML = list.map(function (s) {
      return '<div class="ds-demo__cap" style="color:#6b6b66">' + (s.label || ((s.theme === 'light' ? 'Light' : 'Dark') + ' · ' + (s.lang === 'th' ? 'ไทย' : 'English') + ' · ' + s.w + '×' + s.h + (s.density ? ' · ' + s.density : ''))) + '</div>' +
        '<div class="ds ds-demo__screen" data-theme="' + s.theme + '" lang="' + s.lang + '" data-density="' + (s.density || 'compact') + '" style="width:' + s.w + 'px;height:' + s.h + 'px">' +
        '<div class="ds-demo__tag">' + (s.lang === 'th' ? 'ข้อมูลตัวอย่าง' : 'Sample data') + '</div>' + fn(s.lang, tr(s.lang), s) + '</div>';
    }).join('<div style="height:20px"></div>');
  }

  window.PosDS = { version: '0.1.0', icon: icon, money: money, qty: qty, date: date, tr: tr, pair: pair, screens: screens, ui: ui,
    sample: { cart: cart, products: products, cats: cats, total: total, received: 1500 } };
})();

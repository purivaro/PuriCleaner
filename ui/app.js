'use strict';

/* ─── Bridge: desktop app (Tauri) or local server ─────────────── */

const TAURI = window.__TAURI__ && window.__TAURI__.core;
if (TAURI) document.documentElement.classList.add('is-tauri');
// The server only listens on this computer, so the browser's OS is the scanned one.
const OS = /Windows/.test(navigator.userAgent) ? 'windows' : 'macos';
document.documentElement.classList.add('is-' + OS);

function store(key, value) {
  try {
    if (value === undefined) return sessionStorage.getItem(key);
    sessionStorage.setItem(key, value);
  } catch (_) { return null; }
}
function keep(key, value) {
  try {
    if (value === undefined) return localStorage.getItem(key);
    localStorage.setItem(key, value);
  } catch (_) { return null; }
}

// Read link parameters once; the token is then removed from the address bar.
const HASH = new URLSearchParams(location.hash.slice(1));
let TOKEN = HASH.get('t');
if (TOKEN) {
  store('puri-token', TOKEN);
  history.replaceState(null, '', location.pathname);
} else {
  TOKEN = store('puri-token');
}

async function api(cmd, args = {}) {
  if (TAURI) return TAURI.invoke('api', { cmd, args });
  const res = await fetch('/api/' + cmd, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'X-Puri-Token': TOKEN || '' },
    body: JSON.stringify(args),
  });
  if (res.status === 403) throw new Error(t('err_token'));
  const json = await res.json();
  if (!res.ok) throw new Error(json.error || res.statusText);
  return json;
}

/* ─── Text ────────────────────────────────────────────────────── */

const I18N = {
  th: {
    tagline: 'ดูว่าพื้นที่หายไปไหน แล้วคืนพื้นที่อย่างปลอดภัย',
    tab_disk: 'พื้นที่ดิสก์',
    tab_mem: 'หน่วยความจำ',
    tab_apps: 'แอป',
    apps_count: 'แอปที่ติดตั้ง',
    apps_unit: 'แอป',
    apps_running_n: 'เปิดอยู่ {n} แอป',
    apps_size: 'พื้นที่ของตัวแอป',
    apps_size_note: 'รวมทุกแอปในโฟลเดอร์ Applications',
    apps_data: 'ไฟล์ที่แอปเก็บไว้',
    apps_data_note: 'ข้อมูล แคช และการตั้งค่าใน Library',
    apps_unused: 'ไม่ได้เปิดเกิน 3 เดือน',
    apps_unused_note: 'รวมไฟล์ที่แอปเหล่านั้นเก็บไว้',
    apps_title: 'แอปที่ติดตั้ง',
    apps_sub: 'ถอนการติดตั้งแบบหมดจด: เอาตัวแอปและไฟล์ที่แอปทิ้งไว้ออกพร้อมกัน ทุกอย่างย้ายลงถังขยะ กู้คืนได้',
    apps_search: 'ค้นหาแอป',
    apps_refresh: 'อ่านรายการใหม่',
    sort_size: 'ขนาด',
    sort_used: 'ไม่ได้ใช้นานสุด',
    sort_name: 'ชื่อ',
    apps_loading: 'กำลังอ่านแอปและไฟล์ที่แอปเก็บไว้… {d} / {n}',
    apps_none: 'ไม่พบแอปที่ถอนการติดตั้งได้',
    apps_none_match: 'ไม่พบแอปที่ตรงกับคำค้น',
    apps_used: 'ใช้ล่าสุด {t}',
    apps_installed: 'ติดตั้ง {t}',
    apps_split: 'แอป {app} + ไฟล์ {data}',
    apps_open: 'เปิดอยู่',
    apps_unused_tag: 'ไม่ได้ใช้นาน',
    apps_self: 'แอปนี้เอง',
    uninstall: 'ถอนการติดตั้ง',
    un_title: 'ถอนการติดตั้ง {name}',
    un_app: 'ตัวแอป',
    kind_support: 'ข้อมูลของแอป',
    kind_container: 'ข้อมูลของแอป (sandbox)',
    kind_cache: 'แคช',
    kind_web: 'ข้อมูลเว็บและคุกกี้',
    kind_prefs: 'การตั้งค่า',
    kind_state: 'สถานะหน้าต่าง',
    kind_logs: 'บันทึกการทำงาน (log)',
    kind_launch: 'ตัวเปิดอัตโนมัติ',
    kind_helper: 'โปรแกรมช่วยของระบบ',
    kind_folder: 'โฟลเดอร์ของแอป',
    kind_install: 'โฟลเดอร์ที่ติดตั้ง ถ้ายังค้างหลังถอน',
    un_admin_tag: 'โฟลเดอร์ระบบ',
    un_name_tag: 'ชื่อตรงกัน ตรวจก่อน',
    un_review_note: 'รายการที่ติดป้าย "ชื่อตรงกัน" จับได้จากชื่อแอปเท่านั้น จึงไม่ได้ติ๊กไว้ให้ ดูให้แน่ใจว่าเป็นของแอปนี้ก่อนเลือก',
    un_running: 'แอปนี้เปิดอยู่ จะสั่งปิดให้ก่อน ถ้ามีงานที่ยังไม่บันทึก แอปจะถามก่อน',
    un_extension: 'แอปนี้มีส่วนขยายระบบ (System Extension) macOS อาจถามให้ยืนยันการเอาออก หรือปิดเองได้ที่ System Settings › General › Login Items & Extensions',
    un_nothing_left: 'ไม่พบไฟล์ที่แอปนี้เก็บไว้ใน Library',
    un_sum: 'ย้ายลงถังขยะ <b>{size}</b> — ตัวแอปและไฟล์ {n} รายการ',
    un_admin_note: 'ไฟล์ในโฟลเดอร์ระบบ {n} รายการจะถูกลบถาวร และ macOS จะขอรหัสผ่านผู้ดูแลเครื่อง',
    un_data_warn: 'ที่เลือกไว้มีข้อมูลของแอป {size} เช่น แชต เอกสาร หรือไฟล์งานที่เก็บในเครื่อง ถ้ายังต้องใช้ให้เอาติ๊กออก ถ้าเผลอลบ ยังกู้คืนจากถังขยะได้จนกว่าจะล้างถัง',
    un_running_title: 'กำลังถอน {name}…',
    step_quit: 'ปิดแอป',
    step_remove: 'ย้ายแอปและไฟล์ลงถังขยะ',
    step_uninstaller: 'เปิดตัวถอนการติดตั้งของแอป',
    step_waiting: 'รอให้ถอนการติดตั้งเสร็จ',
    step_files: 'ย้ายไฟล์ลงถังขยะ',
    un_quit_hint: 'ถ้าแอปถามให้บันทึกงาน ตอบที่หน้าต่างของแอปได้เลย',
    un_uninstaller_hint: '',
    un_waiting_hint: '',
    un_stop: 'หยุดรอ',
    un_done_title: 'ถอน {name} แล้ว',
    un_figure: 'เอาออกแล้ว',
    un_still_open_title: '{name} ยังเปิดอยู่',
    un_still_open: 'แอปอาจกำลังถามให้บันทึกงาน บันทึกแล้วปิดแอป จากนั้นกดถอนการติดตั้งอีกครั้ง ยังไม่ได้ลบอะไรเลย',
    un_cancelled_title: 'ยกเลิกแล้ว',
    un_cancelled: 'ยังไม่ได้ลบอะไรเลย',
    orphans_title: 'ไฟล์ค้างของแอปที่ลบไปแล้ว',
    orphans_sub: 'ตั้งชื่อตามแอปที่ไม่มีในเครื่องแล้ว และไม่มีใครแตะมาเกิน 30 วัน ดูชื่อแล้วเลือกเอง',
    orphan_items: '{n} รายการ',
    orphan_changed: 'แก้ไขล่าสุด {t}',
    orphans_btn: 'เลือกรายการที่จะเอาออก',
    orphans_btn_n: 'ย้ายลงถังขยะ {size}',
    orphans_confirm: 'ย้ายไฟล์ค้าง {size} ลงถังขยะ?',
    orphans_confirm_d: 'ไฟล์เหล่านี้ไม่มีแอปไหนในเครื่องใช้แล้ว กู้คืนจากถังขยะได้จนกว่าจะล้างถัง',
    orphans_running: 'กำลังย้ายไฟล์ค้าง…',
    orphans_done_title: 'ย้ายไฟล์ค้างแล้ว',
    confirm_move: 'ย้ายลงถังขยะ',
    mem_used: 'แรมที่ใช้อยู่',
    mem_of: 'จาก {total}',
    mem_pressure: 'ความตึงของแรม',
    p_normal: 'ปกติ',
    p_warn: 'เริ่มตึง',
    p_critical: 'ตึงมาก',
    p_normal_d: 'แรมพอใช้ ปิดแอปไม่ได้ทำให้เร็วขึ้น',
    p_warn_d: 'เริ่มต้องพึ่ง swap ปิดแอปที่ไม่ใช้จะช่วยได้',
    p_critical_d: 'แรมไม่พอ เครื่องจะหน่วง ควรปิดแอปที่ไม่ใช้',
    mem_swap: 'Swap (แรมสำรองบนดิสก์)',
    mem_swap_note: 'ใช้มาก แปลว่าที่ผ่านมาแรมไม่พอ',
    mem_quittable: 'แอปที่ปิดได้',
    mem_quittable_note: 'แรมรวมของแอปที่เปิดอยู่',
    n_apps: '{n} แอป',
    mem_apps: 'แอปที่เปิดอยู่',
    mem_apps_sub: 'เรียงตามแรมที่ใช้ ปิดแอปที่ไม่ได้ใช้เพื่อคืนแรม ถ้ามีงานค้าง แอปจะถามให้บันทึกก่อน',
    mem_live: 'อัปเดตสด',
    mem_background: 'โปรแกรมเบื้องหลังและระบบ',
    mem_background_note: 'เป็นส่วนของระบบหรือโปรแกรมที่ทำงานเบื้องหลัง Puri Cleaner จะไม่ปิดให้ เพราะอาจทำให้เครื่องหรือโปรแกรมอื่นทำงานผิดปกติ',
    mem_procs: '{n} process',
    mem_proc1: '1 process',
    mem_running: 'เปิดมา {t}',
    mem_cpu: 'CPU {n}%',
    mem_idle: 'ไม่ได้ทำงาน',
    mem_empty: 'ไม่มีแอปที่ปิดได้ในตอนนี้',
    dur_m: '{n} นาที',
    dur_h: '{n} ชม.',
    dur_d: '{n} วัน',
    quit: 'ปิดแอป',
    quitting: 'กำลังปิด…',
    quit_title: 'ปิด {name}?',
    quit_text: 'แอปจะปิดแบบปกติ เหมือนกด Quit เอง ถ้ามีงานที่ยังไม่บันทึก แอปจะถามก่อน',
    quit_frees: 'จะได้แรมคืนประมาณ {size}',
    quit_done: 'ปิด {name} แล้ว ได้แรมคืน {size}',
    quit_pending: '{name} ยังเปิดอยู่ อาจกำลังถามให้บันทึกงาน ลองดูที่หน้าต่างของแอป',
    theme_dark: 'เปลี่ยนเป็นโหมดมืด',
    theme_light: 'เปลี่ยนเป็นโหมดสว่าง',
    rescan: 'สแกนใหม่',
    scanning_title: 'กำลังสแกนทั้งเครื่อง…',
    files_unit: 'ไฟล์',
    scan_sysinfo: 'กำลังอ่านข้อมูล Simulator และแอป…',
    scan_stalled: 'ไม่มีความคืบหน้าสักพักแล้ว macOS อาจกำลังถามสิทธิ์เข้าถึงโฟลเดอร์ ลองดูหน้าต่างแจ้งเตือนแล้วกดอนุญาต',
    scan_note: 'อ่านอย่างเดียว ยังไม่มีการลบอะไร ไฟล์ iCloud ที่ไม่ได้อยู่ในเครื่องจะถูกข้ามโดยไม่ดาวน์โหลด',
    cancel: 'ยกเลิก',
    close: 'ปิด',
    free_after: 'ว่างหลังล้าง',
    safe_to_remove: 'ลบได้ปลอดภัย',
    needs_review: 'ควรดูก่อนลบ',
    not_scanned: 'macOS และอื่น ๆ',
    system_apps: 'ลบไม่ได้',
    n_groups: '{n} หมวด',
    explorer: 'แผนที่พื้นที่',
    explorer_sub: 'ทั้งเครื่อง ขนาดกล่องตามพื้นที่ที่ใช้ คลิกเพื่อเปิดดูข้างใน',
    view_category: 'ตามประเภท',
    view_cleanup: 'ตามการล้าง',
    home: 'โฮม',
    recommendations: 'คำแนะนำ',
    recs_sub: 'เลือกรายการที่จะล้าง แตะแถวเพื่อดูไฟล์ข้างใน',
    group_safe: 'ลบได้ปลอดภัย',
    group_review: 'ดูก่อนลบ',
    n_items: '{n} รายการ',
    select_safe: 'เลือกที่ปลอดภัย',
    clear_selection: 'ล้างการเลือก',
    copy_script: 'สคริปต์',
    clean_up: 'ล้างพื้นที่…',
    selected: 'ที่เลือก',
    sel_none: 'ยังไม่ได้เลือกรายการ',
    sel_summary: '· {r} หมวด {i} รายการ',
    alt_command: 'หรือใช้คำสั่งของเครื่องมือเอง',
    select_all: 'เลือกทั้งหมด',
    files_count: '{n} ไฟล์',
    modified: 'แก้ล่าสุด {when}',
    show_in_map: 'ดูในแผนที่',
    reveal: 'เปิดใน Finder',
    open_folder: 'เปิดโฟลเดอร์นี้',
    show_rec: 'ดูคำแนะนำ',
    small_files: 'ไฟล์เล็ก {n} ไฟล์',
    more_items: 'อีก {n} รายการ',
    of_folder: 'ของโฟลเดอร์นี้',
    tt_click: 'คลิกเพื่อเปิด · คลิกขวาดูเมนู',
    tt_right: 'คลิกขวาดูเมนู',
    empty_folder: 'โฟลเดอร์นี้ว่าง',
    cat_docs: 'เอกสารและสื่อ',
    cat_dev: 'เครื่องมือและอื่น ๆ',
    cat_code: 'โปรเจคโค้ด',
    cat_sdk: 'SDK iOS และ Android',
    cat_ai: 'โมเดล AI',
    cat_pkg: 'แคชแพ็กเกจ',
    cat_apps: 'แอปพลิเคชัน',
    cat_sys: 'ระบบ',
    this_mac: 'Mac เครื่องนี้',
    macos_tile: 'macOS และพื้นที่ที่อ่านไม่ได้',
    macos_tip: 'ตัวระบบ macOS, สแนปช็อต, หน่วยความจำเสมือน และโฟลเดอร์ที่ไม่มีสิทธิ์อ่าน ส่วนนี้ลบเองไม่ได้',
    last_opened: 'เปิดล่าสุด {when}',
    never_opened: 'ไม่มีประวัติการเปิด',
    used_by_sims: 'ใช้โดย Simulator {n} เครื่อง',
    no_sims: 'ไม่มี Simulator ที่ใช้อยู่',
    note_admin: 'เป็นไฟล์ของระบบ ต้องใส่รหัสผ่านเครื่อง และจะลบถาวรเสมอ',
    note_finder: 'แอปจะถูกย้ายไปถังขยะเสมอ กู้คืนได้',
    note_command: 'ลบผ่านเครื่องมือของ Xcode (xcrun simctl) และลบถาวร',
    lg_safe: 'ลบได้ปลอดภัย',
    lg_review: 'ดูก่อนลบ',
    lg_system: 'macOS และอื่น ๆ',
    lg_free: 'ว่าง',
    lg_selected: 'จะได้คืน',
    fda_text: 'อ่านไม่ได้ {n} โฟลเดอร์ ถ้าอนุญาต Full Disk Access ตัวเลขจะครบขึ้น',
    fda_button: 'เปิดการตั้งค่า',
    recs_empty_t: 'สะอาดแล้ว',
    recs_empty: 'ไม่พบสิ่งที่ควรล้าง',
    script_title: 'สคริปต์ล้างพื้นที่',
    script_text: 'คัดลอกไปวางใน Terminal ได้ อ่านก่อนรันทุกครั้ง คำสั่ง rm -rf ลบถาวร ไม่ผ่านถังขยะ',
    copy: 'คัดลอก',
    copied: 'คัดลอกแล้ว',
    clean_title: 'ล้างพื้นที่ {size}',
    clean_how: 'ลบอย่างไร',
    mode_trash: 'ย้ายไปถังขยะ',
    mode_trash_d: 'กู้คืนได้ แต่พื้นที่จะคืนเมื่อล้างถังขยะ',
    mode_delete: 'ลบถาวร',
    mode_delete_d: 'ได้พื้นที่คืนทันที กู้คืนไม่ได้',
    review_warn: 'มี {n} หมวดที่ควรดูก่อนลบ ตรวจในรายการให้แน่ใจว่าไม่ต้องใช้แล้ว',
    confirm_trash: 'ย้ายไปถังขยะ',
    confirm_delete: 'ลบถาวร',
    cleaning: 'กำลังล้าง…',
    cleaning_n: '{d} จาก {n} รายการ',
    done_title: 'เรียบร้อย',
    done_freed: 'ได้พื้นที่คืน',
    done_also_moved: 'และย้ายไปถังขยะ {size}',
    done_moved: 'ย้ายไปถังขยะแล้ว',
    done_trash_note: 'พื้นที่จะคืนเมื่อล้างถังขยะ (Finder › ล้างถังขยะ)',
    done_errors: 'ลบไม่ได้ {n} รายการ',
    err_token: 'ลิงก์หมดอายุ ให้เปิดจากหน้าต่าง Terminal ที่รัน Puri Cleaner อีกครั้ง',
    err_generic: 'มีข้อผิดพลาด: {msg}',
    scan_failed: 'สแกนไม่สำเร็จ: {msg}',
    retry: 'ลองใหม่',
    scanned_in: 'สแกน {files} ไฟล์ใน {sec} วินาที',
    up: 'ขึ้นไป',
  },
  en: {
    tagline: 'See where your space went, and get it back safely',
    tab_disk: 'Disk',
    tab_mem: 'Memory',
    tab_apps: 'Apps',
    apps_count: 'Installed apps',
    apps_unit: 'apps',
    apps_running_n: '{n} open now',
    apps_size: 'Apps themselves',
    apps_size_note: 'Everything in the Applications folders',
    apps_data: 'Files apps keep',
    apps_data_note: 'Data, caches and settings in Library',
    apps_unused: 'Not opened in 3 months',
    apps_unused_note: 'Including the files those apps keep',
    apps_title: 'Installed apps',
    apps_sub: 'A clean uninstall removes the app and the files it left behind together. Everything goes to the Trash, so it can be restored.',
    apps_search: 'Search apps',
    apps_refresh: 'Read the list again',
    sort_size: 'Size',
    sort_used: 'Least used',
    sort_name: 'Name',
    apps_loading: 'Reading apps and the files they keep… {d} / {n}',
    apps_none: 'No apps that can be uninstalled were found',
    apps_none_match: 'No app matches your search',
    apps_used: 'last used {t}',
    apps_installed: 'installed {t}',
    apps_split: 'app {app} + files {data}',
    apps_open: 'Open',
    apps_unused_tag: 'Unused',
    apps_self: 'This app',
    uninstall: 'Uninstall',
    un_title: 'Uninstall {name}',
    un_app: 'The app',
    kind_support: 'App data',
    kind_container: 'App data (sandbox)',
    kind_cache: 'Cache',
    kind_web: 'Web data & cookies',
    kind_prefs: 'Settings',
    kind_state: 'Window state',
    kind_logs: 'Logs',
    kind_launch: 'Starts automatically',
    kind_helper: 'System helper',
    kind_folder: 'App folder',
    kind_install: 'Install folder, if still there afterwards',
    un_admin_tag: 'system folder',
    un_name_tag: 'name match, check first',
    un_review_note: 'Items tagged "name match" were found by the app’s name only, so they are not ticked. Make sure they belong to this app before selecting them.',
    un_running: 'This app is open. It is asked to quit first; an app with unsaved work asks before closing.',
    un_extension: 'This app has a system extension. macOS may ask you to confirm removing it, or turn it off in System Settings › General › Login Items & Extensions.',
    un_nothing_left: 'No files kept by this app were found in Library.',
    un_sum: 'Moves <b>{size}</b> to the Trash: the app and {n} items',
    un_admin_note: '{n} items in system folders are deleted permanently, and macOS asks for an administrator password.',
    un_data_warn: 'The selection includes {size} of app data, such as chats, documents or work kept on this Mac. Untick it if you still need it. Until you empty the Trash, it can be restored.',
    un_running_title: 'Uninstalling {name}…',
    step_quit: 'Quit the app',
    step_remove: 'Move the app and its files to the Trash',
    step_uninstaller: 'Open the app’s uninstaller',
    step_waiting: 'Wait for the uninstaller to finish',
    step_files: 'Move the files to the Trash',
    un_quit_hint: 'If the app asks to save your work, answer it in the app’s window.',
    un_uninstaller_hint: '',
    un_waiting_hint: '',
    un_stop: 'Stop waiting',
    un_done_title: '{name} is uninstalled',
    un_figure: 'removed',
    un_still_open_title: '{name} is still open',
    un_still_open: 'It may be asking to save your work. Save, quit the app, then press Uninstall again. Nothing was removed.',
    un_cancelled_title: 'Cancelled',
    un_cancelled: 'Nothing was removed.',
    orphans_title: 'Leftovers of deleted apps',
    orphans_sub: 'Named after apps that are no longer on this Mac and untouched for 30+ days. Check the names, then choose.',
    orphan_items: '{n} items',
    orphan_changed: 'last changed {t}',
    orphans_btn: 'Select items to remove',
    orphans_btn_n: 'Move {size} to the Trash',
    orphans_confirm: 'Move {size} of leftovers to the Trash?',
    orphans_confirm_d: 'No app on this Mac uses these files any more. Until you empty the Trash, they can be restored.',
    orphans_running: 'Moving leftovers…',
    orphans_done_title: 'Leftovers moved',
    confirm_move: 'Move to Trash',
    mem_used: 'RAM in use',
    mem_of: 'of {total}',
    mem_pressure: 'Memory pressure',
    p_normal: 'Normal',
    p_warn: 'Getting tight',
    p_critical: 'Critical',
    p_normal_d: 'Plenty of RAM; quitting apps will not speed things up.',
    p_warn_d: 'Swap is being used; quitting unused apps will help.',
    p_critical_d: 'Out of RAM; the Mac will lag. Quit apps you are not using.',
    mem_swap: 'Swap (RAM on disk)',
    mem_swap_note: 'A lot means RAM ran short recently',
    mem_quittable: 'Apps you can quit',
    mem_quittable_note: 'RAM held by open apps',
    n_apps: '{n} apps',
    mem_apps: 'Open apps',
    mem_apps_sub: 'Sorted by RAM. Quit the ones you are not using; an app with unsaved work asks first.',
    mem_live: 'Live',
    mem_background: 'Background & system',
    mem_background_note: 'System parts and background programs. Puri Cleaner does not quit these, since that can break the system or other apps.',
    mem_procs: '{n} processes',
    mem_proc1: '1 process',
    mem_running: 'open {t}',
    mem_cpu: 'CPU {n}%',
    mem_idle: 'Idle',
    mem_empty: 'No apps to quit right now.',
    dur_m: '{n} min',
    dur_h: '{n} h',
    dur_d: '{n} days',
    quit: 'Quit',
    quitting: 'Quitting…',
    quit_title: 'Quit {name}?',
    quit_text: 'The app quits normally, just like choosing Quit. If it has unsaved work, it asks first.',
    quit_frees: 'Frees about {size} of RAM',
    quit_done: 'Quit {name} — {size} of RAM freed',
    quit_pending: '{name} is still open; it may be asking to save. Check its window.',
    theme_dark: 'Switch to dark mode',
    theme_light: 'Switch to light mode',
    rescan: 'Rescan',
    scanning_title: 'Scanning your Mac…',
    files_unit: 'files',
    scan_sysinfo: 'Reading simulator and app details…',
    scan_stalled: 'No progress for a while. macOS may be asking for folder access; look for a permission dialog and allow it.',
    scan_note: 'Read-only, nothing is deleted. iCloud files that are not on this Mac are skipped, never downloaded.',
    cancel: 'Cancel',
    close: 'Close',
    free_after: 'Free after cleanup',
    safe_to_remove: 'Safe to remove',
    needs_review: 'Needs review',
    not_scanned: 'macOS & other',
    system_apps: 'not removable',
    n_groups: '{n} items',
    explorer: 'Explorer',
    explorer_sub: 'Your whole Mac, sized by space used. Click a tile to open it.',
    view_category: 'Category',
    view_cleanup: 'Cleanup',
    home: 'Home',
    recommendations: 'Recommendations',
    recs_sub: 'Select items to clean. Click a row to see what is inside.',
    group_safe: 'Safe to remove',
    group_review: 'Review first',
    n_items: '{n} items',
    select_safe: 'Select safe',
    clear_selection: 'Clear',
    copy_script: 'Script',
    clean_up: 'Clean up…',
    selected: 'selected',
    sel_none: 'Nothing selected',
    sel_summary: '· {r} groups, {i} items',
    alt_command: 'Or use the tool’s own command',
    select_all: 'Select all',
    files_count: '{n} files',
    modified: 'changed {when}',
    show_in_map: 'Show in map',
    reveal: 'Show in Finder',
    open_folder: 'Open this folder',
    show_rec: 'Show recommendation',
    small_files: '{n} small files',
    more_items: '{n} more',
    of_folder: 'of this folder',
    tt_click: 'Click to open · right-click for menu',
    tt_right: 'Right-click for menu',
    empty_folder: 'This folder is empty',
    cat_docs: 'Documents & media',
    cat_dev: 'Dev tools & other',
    cat_code: 'Code projects',
    cat_sdk: 'iOS & Android SDKs',
    cat_ai: 'AI models',
    cat_pkg: 'Package caches',
    cat_apps: 'Applications',
    cat_sys: 'System',
    this_mac: 'This Mac',
    macos_tile: 'macOS & unreadable space',
    macos_tip: 'macOS itself, snapshots, virtual memory and folders we have no permission to read. Not something you can clean by hand.',
    last_opened: 'last opened {when}',
    never_opened: 'never opened',
    used_by_sims: 'used by {n} simulators',
    no_sims: 'no simulators use it',
    note_admin: 'System files: your Mac password is needed, and they are always deleted permanently.',
    note_finder: 'Apps always go to the Trash, so you can restore them.',
    note_command: 'Removed with Xcode’s own tool (xcrun simctl), permanently.',
    lg_safe: 'Safe to remove',
    lg_review: 'Review first',
    lg_system: 'macOS & other',
    lg_free: 'Free',
    lg_selected: 'To reclaim',
    fda_text: '{n} folders could not be read. Allow Full Disk Access for complete numbers.',
    fda_button: 'Open Settings',
    recs_empty_t: 'All clean',
    recs_empty: 'Nothing worth cleaning was found.',
    script_title: 'Cleanup script',
    script_text: 'Paste into Terminal. Always read it first: rm -rf deletes permanently and skips the Trash.',
    copy: 'Copy',
    copied: 'Copied',
    clean_title: 'Clean up {size}',
    clean_how: 'How to remove',
    mode_trash: 'Move to Trash',
    mode_trash_d: 'Recoverable. Space comes back when you empty the Trash.',
    mode_delete: 'Delete permanently',
    mode_delete_d: 'Space comes back right away. Cannot be undone.',
    review_warn: '{n} of these need a look first. Check their lists to be sure you no longer need them.',
    confirm_trash: 'Move to Trash',
    confirm_delete: 'Delete permanently',
    cleaning: 'Cleaning up…',
    cleaning_n: '{d} of {n} items',
    done_title: 'Done',
    done_freed: 'Space reclaimed',
    done_also_moved: 'plus {size} moved to the Trash',
    done_moved: 'Moved to Trash',
    done_trash_note: 'Space comes back when you empty the Trash (Finder › Empty Trash).',
    done_errors: '{n} items could not be removed',
    err_token: 'This link has expired. Open it again from the Terminal running Puri Cleaner.',
    err_generic: 'Something went wrong: {msg}',
    scan_failed: 'Scan failed: {msg}',
    retry: 'Try again',
    scanned_in: 'Scanned {files} files in {sec}s',
    up: 'Up',
  },
};

/* Theme: follows the system until the user picks one with the button. */
function systemDark() { return matchMedia('(prefers-color-scheme: dark)').matches; }
function currentTheme() { return document.documentElement.dataset.theme || (systemDark() ? 'dark' : 'light'); }
function applyTheme(theme) {
  if (theme) document.documentElement.dataset.theme = theme;
  const btn = document.getElementById('themeBtn');
  if (btn) {
    const next = currentTheme() === 'dark' ? 'light' : 'dark';
    btn.title = t(next === 'dark' ? 'theme_dark' : 'theme_light');
    btn.setAttribute('aria-label', btn.title);
  }
}
// Set early so the page never flashes the wrong theme; the button label comes later.
if (keep('puri-theme')) document.documentElement.dataset.theme = keep('puri-theme');

/* Wording that differs on Windows (Explorer, Recycle Bin, UAC, OneDrive). */
const I18N_WIN = {
  th: {
    apps_size_note: 'ตามขนาดที่แต่ละแอปแจ้งไว้กับ Windows',
    apps_data_note: 'โฟลเดอร์ใน AppData และ ProgramData',
    apps_sub: 'ถอนการติดตั้งแบบหมดจด: ใช้ตัวถอนการติดตั้งของแอปเอง แล้วเก็บโฟลเดอร์ที่ค้างไว้ให้ ไฟล์ที่เหลือย้ายไปถังรีไซเคิล กู้คืนได้ · แอปจาก Microsoft Store ถอนได้ที่ Settings › Apps',
    sort_used: 'ติดตั้งนานสุด',
    un_app: 'ตัวแอป (ใช้ตัวถอนการติดตั้งของแอปเอง)',
    kind_support: 'ข้อมูลของแอป (AppData)',
    un_running: 'แอปนี้เปิดอยู่ ตัวถอนการติดตั้งอาจขอให้ปิดก่อน',
    un_win_flow: 'Puri Cleaner จะเปิดตัวถอนการติดตั้งของแอปเอง ทำตามขั้นตอนในหน้าต่างนั้นให้เสร็จ แล้ว Puri Cleaner จะเก็บโฟลเดอร์ที่เหลือที่เลือกไว้ให้',
    un_nothing_left: 'ไม่พบโฟลเดอร์ที่แอปนี้ทิ้งไว้',
    un_sum: 'ถอนตัวแอปและเก็บโฟลเดอร์ {n} รายการ รวม <b>{size}</b>',
    un_admin_note: 'โฟลเดอร์ใน Program Files / ProgramData {n} รายการจะถูกลบถาวร Windows จะขอสิทธิ์ผู้ดูแลระบบ',
    un_data_warn: 'ที่เลือกไว้มีข้อมูลของแอป {size} ถ้ายังต้องใช้ให้เอาติ๊กออก ถ้าเผลอลบ ยังกู้คืนจากถังรีไซเคิลได้จนกว่าจะล้างถัง',
    step_files: 'ย้ายโฟลเดอร์ที่เหลือไปถังรีไซเคิล',
    un_uninstaller_hint: 'ทำตามขั้นตอนในหน้าต่างถอนการติดตั้งของแอป ถ้า Windows ถามสิทธิ์ผู้ดูแลระบบ ให้กด Yes',
    un_waiting_hint: 'ตัวถอนการติดตั้งยังไม่เสร็จ ถ้าหน้าต่างของมันยังเปิดอยู่ ทำให้เสร็จก่อน ถ้ากดยกเลิกไปแล้ว กด "หยุดรอ" ได้',
    un_cancelled: 'ยังไม่ได้ลบโฟลเดอร์ใด ๆ',
    scanning_title: 'กำลังสแกนทั้งเครื่อง…',
    scan_stalled: 'ไม่มีความคืบหน้าสักพักแล้ว อาจมีโฟลเดอร์ที่ตอบช้าหรือรอสิทธิ์อยู่',
    scan_note: 'อ่านอย่างเดียว ยังไม่มีการลบอะไร ไฟล์ OneDrive ที่ไม่ได้อยู่ในเครื่องจะถูกข้ามโดยไม่ดาวน์โหลด',
    not_scanned: 'Windows และอื่น ๆ',
    lg_system: 'Windows และอื่น ๆ',
    reveal: 'เปิดใน File Explorer',
    this_mac: 'PC เครื่องนี้',
    macos_tile: 'ไฟล์ระบบและพื้นที่ที่อ่านไม่ได้',
    macos_tip: 'pagefile.sys, hiberfil.sys, จุดคืนค่าระบบ (System Restore) และโฟลเดอร์ที่ต้องใช้สิทธิ์ผู้ดูแลระบบถึงจะอ่านได้ ถ้าเปิดโปรแกรมแบบ Run as administrator จะเห็นละเอียดขึ้น',
    note_admin: 'อยู่นอกโฟลเดอร์ผู้ใช้ Windows จะขอสิทธิ์ผู้ดูแลระบบ (UAC) และจะลบถาวรเสมอ',
    note_finder: 'จะถูกย้ายไปถังรีไซเคิลเสมอ กู้คืนได้',
    fda_text: 'อ่านไม่ได้ {n} โฟลเดอร์ ถ้าเปิดโปรแกรมแบบ Run as administrator ตัวเลขจะครบขึ้น',
    admin_text: 'อ่านไม่ได้ {n} โฟลเดอร์ (รวมโฟลเดอร์ระบบ เช่น C:\\Windows\\Temp) สแกนแบบผู้ดูแลระบบเพื่อเห็นครบและได้คำแนะนำเพิ่ม เช่น ล้าง WinSxS',
    admin_button: 'สแกนแบบผู้ดูแลระบบ',
    admin_relaunching: 'กำลังเปิดใหม่แบบผู้ดูแลระบบ…',
    admin_relaunching_d: 'หน้าต่างนี้จะปิดเอง แล้ว Puri Cleaner จะเปิดขึ้นมาใหม่และสแกนอีกครั้ง',
    admin_cancelled: 'ยกเลิกการขอสิทธิ์ผู้ดูแลระบบแล้ว',
    as_admin: 'ผู้ดูแลระบบ',
    scan_sysinfo: 'กำลังวิเคราะห์ WinSxS ด้วย DISM (อาจใช้ 1–2 นาที)…',
    script_text: 'คัดลอกไปวางใน PowerShell ได้ อ่านก่อนรันทุกครั้ง คำสั่งเหล่านี้ลบถาวร ไม่ผ่านถังรีไซเคิล',
    mode_trash: 'ย้ายไปถังรีไซเคิล',
    mode_trash_d: 'กู้คืนได้ แต่พื้นที่จะคืนเมื่อล้างถังรีไซเคิล',
    confirm_trash: 'ย้ายไปถังรีไซเคิล',
    done_moved: 'ย้ายไปถังรีไซเคิลแล้ว',
    done_trash_note: 'พื้นที่จะคืนเมื่อล้างถังรีไซเคิล (คลิกขวาที่ Recycle Bin › Empty Recycle Bin)',
    lg_free: 'ว่าง',
  },
  en: {
    apps_size_note: 'As each app reports it to Windows',
    apps_data_note: 'Folders in AppData and ProgramData',
    apps_sub: 'A clean uninstall runs the app’s own uninstaller, then removes the folders it left behind (to the Recycle Bin, so they can be restored). Microsoft Store apps are removed in Settings › Apps.',
    sort_used: 'Oldest install',
    un_app: 'The app (with its own uninstaller)',
    kind_support: 'App data (AppData)',
    un_running: 'This app is open. Its uninstaller may ask you to close it first.',
    un_win_flow: 'Puri Cleaner opens the app’s own uninstaller. Finish the steps in its window; Puri Cleaner then removes the leftover folders you selected.',
    un_nothing_left: 'No leftover folders of this app were found.',
    un_sum: 'Uninstalls the app and removes {n} folders, <b>{size}</b> in all',
    un_admin_note: '{n} folders in Program Files / ProgramData are deleted permanently. Windows asks for administrator permission.',
    un_data_warn: 'The selection includes {size} of app data. Untick it if you still need it. Until you empty the Recycle Bin, it can be restored.',
    step_files: 'Move leftover folders to the Recycle Bin',
    un_uninstaller_hint: 'Follow the steps in the app’s uninstaller. If Windows asks for administrator permission, choose Yes.',
    un_waiting_hint: 'The uninstaller has not finished. If its window is still open, finish it first. If you cancelled it, press Stop waiting.',
    un_cancelled: 'No folders were removed.',
    scan_stalled: 'No progress for a while. A folder may be slow to answer or waiting for permission.',
    scan_note: 'Read-only, nothing is deleted. OneDrive files that are not on this PC are skipped, never downloaded.',
    scanning_title: 'Scanning your PC…',
    explorer_sub: 'Your whole PC, sized by space used. Click a tile to open it.',
    not_scanned: 'Windows & other',
    lg_system: 'Windows & other',
    reveal: 'Show in File Explorer',
    this_mac: 'This PC',
    macos_tile: 'System files & unreadable space',
    macos_tip: 'pagefile.sys, hiberfil.sys, System Restore points and folders that need administrator rights to read. Run as administrator to see more.',
    note_admin: 'Outside your user folder: Windows will ask for administrator permission (UAC), and they are always deleted permanently.',
    note_finder: 'Always moved to the Recycle Bin, so you can restore them.',
    fda_text: '{n} folders could not be read. Run as administrator for complete numbers.',
    admin_text: '{n} folders could not be read, including system ones such as C:\\Windows\\Temp. Scan as administrator to see everything and get more suggestions, like WinSxS cleanup.',
    admin_button: 'Scan as administrator',
    admin_relaunching: 'Reopening as administrator…',
    admin_relaunching_d: 'This window closes by itself; Puri Cleaner opens again and rescans.',
    admin_cancelled: 'Administrator permission was not granted.',
    as_admin: 'administrator',
    scan_sysinfo: 'Analyzing WinSxS with DISM (may take 1–2 minutes)…',
    script_text: 'Paste into PowerShell. Always read it first: these commands delete permanently and skip the Recycle Bin.',
    mode_trash: 'Move to Recycle Bin',
    mode_trash_d: 'Recoverable. Space comes back when you empty the Recycle Bin.',
    confirm_trash: 'Move to Recycle Bin',
    done_moved: 'Moved to the Recycle Bin',
    done_trash_note: 'Space comes back when you empty the Recycle Bin (right-click Recycle Bin › Empty Recycle Bin).',
  },
};

let lang = keep('puri-lang') || ((navigator.language || 'th').startsWith('en') ? 'en' : 'th');

function t(key, vars) {
  const win = OS === 'windows' ? (I18N_WIN[lang] && I18N_WIN[lang][key]) || I18N_WIN.en[key] : null;
  let s = win || (I18N[lang] && I18N[lang][key]) || I18N.en[key] || key;
  if (vars) for (const k in vars) s = s.replace('{' + k + '}', vars[k]);
  return s;
}

/* ─── Helpers ─────────────────────────────────────────────────── */

const $ = (sel, el = document) => el.querySelector(sel);
const $$ = (sel, el = document) => Array.from(el.querySelectorAll(sel));
const CATS = ['docs', 'code', 'sdk', 'ai', 'pkg', 'apps', 'dev', 'sys'];

function esc(s) {
  return String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

function fmt(b) {
  if (b == null || isNaN(b)) return '–';
  const u = ['B', 'KB', 'MB', 'GB', 'TB'];
  let v = b, i = 0;
  while (v >= 1000 && i < u.length - 1) { v /= 1000; i++; }
  if (i === 0) return b + ' B';
  return (v >= 100 ? Math.round(v) : v.toFixed(1)) + ' ' + u[i];
}

function num(n) {
  return Number(n).toLocaleString(lang === 'th' ? 'th-TH' : 'en-US');
}

function ago(sec) {
  if (!sec) return '';
  const diff = sec - Date.now() / 1000;
  const rtf = new Intl.RelativeTimeFormat(lang === 'th' ? 'th' : 'en', { numeric: 'auto' });
  const abs = Math.abs(diff);
  if (abs < 3600) return rtf.format(Math.round(diff / 60), 'minute');
  if (abs < 86400) return rtf.format(Math.round(diff / 3600), 'hour');
  if (abs < 86400 * 45) return rtf.format(Math.round(diff / 86400), 'day');
  if (abs < 86400 * 365) return rtf.format(Math.round(diff / (86400 * 30)), 'month');
  return rtf.format(Math.round(diff / (86400 * 365)), 'year');
}

function el(tag, cls, html) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (html != null) e.innerHTML = html;
  return e;
}

function toast(msg) {
  const tst = el('div', 'toast', esc(msg));
  $('#toasts').append(tst);
  setTimeout(() => { tst.classList.add('out'); setTimeout(() => tst.remove(), 250); }, 2200);
}

const ICON = {
  chevron: '<svg viewBox="0 0 20 20" aria-hidden="true"><path d="m7.5 5 5 5-5 5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  finder: '<svg viewBox="0 0 20 20" aria-hidden="true"><path d="M3 6.5A1.5 1.5 0 0 1 4.5 5h3l1.5 1.5h6.5A1.5 1.5 0 0 1 17 8v6.5a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 3 14.5v-8Z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/></svg>',
  map: '<svg viewBox="0 0 20 20" aria-hidden="true"><rect x="3" y="3" width="14" height="14" rx="2" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M10 3v14M10 9h7" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>',
  open: '<svg viewBox="0 0 20 20" aria-hidden="true"><path d="M4 10h11m-4-4 4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  tip: '<svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="7" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M10 9v4.5M10 6.5v.01" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>',
  check: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12.5 4.5 4.5L19 7.5" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/></svg>',
};

/* ─── State ───────────────────────────────────────────────────── */

const S = {
  // "#tab=mem" in the link opens the memory page directly.
  tab: ['mem', 'apps'].find(x => x === (HASH.get('tab') || keep('puri-tab'))) || 'disk',
  scanVisible: true,
  mem: null,
  memTimer: null,
  quitting: null,
  apps: null,
  appsPolling: false,
  appsSort: keep('puri-apps-sort') || 'size',
  orphanSel: new Set(),
  overview: null,
  node: null,
  recs: [],
  sel: new Set(),
  view: keep('puri-view') || 'category',
  drawer: null,
  spot: null,       // node id to highlight after "show in map"
  fdaHidden: false,
  firstLoad: true,
};

function itemsOf(level) {
  return S.recs.filter(r => !level || r.level === level).flatMap(r => r.items);
}

function recSelected(r) {
  let n = 0, size = 0;
  for (const it of r.items) if (S.sel.has(it.id)) { n++; size += it.size; }
  return { n, size };
}

function selectionTotals() {
  let size = 0, items = 0, recs = 0;
  for (const r of S.recs) {
    const s = recSelected(r);
    if (s.n) { recs++; items += s.n; size += s.size; }
  }
  return { size, items, recs };
}

/* ─── Scan ────────────────────────────────────────────────────── */

function showScanScreen(on) {
  S.scanVisible = on;
  applyTab();
}

/* ─── Tabs: disk / apps / memory ──────────────────────────────── */

function applyTab() {
  const disk = S.tab === 'disk';
  $('#memView').hidden = S.tab !== 'mem';
  $('#appsView').hidden = S.tab !== 'apps';
  $('#scanScreen').hidden = !disk || !S.scanVisible;
  $('#app').hidden = !disk || !!S.scanVisible;
  $('#rescanBtn').hidden = !disk || !!S.scanVisible;
  $$('#tabSeg button').forEach(b => {
    b.classList.toggle('on', b.dataset.tab === S.tab);
    b.setAttribute('aria-selected', b.dataset.tab === S.tab);
  });
  if (S.tab === 'mem') startMem(); else stopMem();
  if (S.tab === 'apps' && !S.apps) loadApps();
  if (disk && S.node) requestAnimationFrame(() => renderTreemap());
}

/* ─── Memory ──────────────────────────────────────────────────── */

const AVATAR = ['#6c5ce7', '#e04f8c', '#1aa3b5', '#4a7ee8', '#e8762d', '#2fa36b', '#9a6bd8', '#d6455d'];

function avatarColor(name) {
  let h = 0;
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return AVATAR[h % AVATAR.length];
}

function duration(sec) {
  if (sec < 3600) return t('dur_m', { n: Math.max(1, Math.round(sec / 60)) });
  if (sec < 86400 * 2) return t('dur_h', { n: Math.round(sec / 3600) });
  return t('dur_d', { n: Math.round(sec / 86400) });
}

async function loadMem() {
  try {
    S.mem = await api('memory');
    renderMem();
  } catch (e) {
    $('#memApps').innerHTML = `<div class="mem-empty">${esc(t('err_generic', { msg: e.message }))}</div>`;
  }
}

function startMem() {
  if (S.memTimer) return;
  loadMem();
  S.memTimer = setInterval(() => {
    if (document.hidden || !$('#modal').hidden || S.quitting) return;
    loadMem();
  }, 4000);
}

function stopMem() {
  clearInterval(S.memTimer);
  S.memTimer = null;
}

function memRow(a, max) {
  const row = el('div', 'mem-row');
  const idle = a.cpu < 0.5 && a.running_for > 3600;
  const meta = [a.processes === 1 ? t('mem_proc1') : t('mem_procs', { n: num(a.processes) })];
  if (a.running_for) meta.push(t('mem_running', { t: duration(a.running_for) }));
  meta.push(t('mem_cpu', { n: a.cpu < 10 ? a.cpu.toFixed(1) : Math.round(a.cpu) }));
  const pct = max ? Math.max(2, 100 * a.memory / max) : 0;
  row.innerHTML = `
    <div class="mem-avatar" style="background:${avatarColor(a.name)}">${esc((a.name.trim()[0] || '?').toUpperCase())}</div>
    <div style="min-width:0">
      <div class="mem-name" title="${esc(a.path || a.name)}">${esc(a.name)}${idle && a.can_quit ? `<span class="mem-tag idle">${esc(t('mem_idle'))}</span>` : ''}</div>
      <div class="mem-meta">${esc(meta.join(' · '))}</div>
    </div>
    <div class="mem-bar"><i style="width:${pct}%"></i></div>
    <div class="mem-size">${fmt(a.memory)}</div>
    ${a.can_quit ? `<button class="btn">${esc(t('quit'))}</button>` : '<div class="mem-lock">—</div>'}`;
  const btn = $('button', row);
  if (btn) btn.onclick = () => askQuit(a, row);
  return row;
}

function renderMem() {
  const m = S.mem;
  if (!m) return;
  const apps = m.apps.filter(a => a.can_quit);
  const others = m.apps.filter(a => !a.can_quit);
  const usedPct = m.total ? 100 * m.used / m.total : 0;
  $('#memUsed').textContent = fmt(m.used);
  $('#memTotal').textContent = t('mem_of', { total: fmt(m.total) });
  const bar = $('#memBar');
  bar.style.width = usedPct.toFixed(1) + '%';
  bar.style.background = m.pressure === 'critical' ? 'var(--danger)' : m.pressure === 'warn' ? 'var(--review)' : 'var(--c-code-ac)';
  const p = $('#memPressure');
  p.className = 'pressure ' + m.pressure;
  p.textContent = t('p_' + m.pressure);
  $('#memPressureNote').textContent = t('p_' + m.pressure + '_d');
  $('#memSwap').textContent = fmt(m.swap_used);
  $('#memSwapTotal').textContent = m.swap_total ? t('mem_of', { total: fmt(m.swap_total) }) : '';
  $('#memQuittable').textContent = fmt(apps.reduce((s, a) => s + a.memory, 0));
  $('#memQuittableN').textContent = t('n_apps', { n: num(apps.length) });

  const max = Math.max(1, ...m.apps.map(a => a.memory));
  const list = $('#memApps');
  list.innerHTML = '';
  if (!apps.length) list.append(el('div', 'mem-empty', esc(t('mem_empty'))));
  for (const a of apps) {
    const row = memRow(a, max);
    if (S.quitting === a.key) row.classList.add('quitting');
    list.append(row);
  }
  const bg = $('#memBg');
  bg.innerHTML = '';
  for (const a of others.slice(0, 40)) bg.append(memRow(a, max));
  $('#memBgTotal').textContent = fmt(others.reduce((s, a) => s + a.memory, 0));
}

function askQuit(a, row) {
  openModal(`
    <h3 class="modal-title" id="modalTitle">${esc(t('quit_title', { name: a.name }))}</h3>
    <p class="modal-text">${esc(t('quit_text'))}</p>
    <p class="modal-text"><b>${esc(t('quit_frees', { size: fmt(a.memory) }))}</b></p>
    <div class="modal-actions">
      <button class="btn" data-close data-autofocus>${esc(t('cancel'))}</button>
      <button class="btn btn-primary" id="confirmQuit">${esc(t('quit'))}</button>
    </div>`);
  $('#confirmQuit').onclick = async () => {
    closeModal();
    S.quitting = a.key;
    row.classList.add('quitting');
    const b = $('button', row);
    if (b) { b.disabled = true; b.textContent = t('quitting'); }
    try {
      const r = await api('quit_app', { key: a.key });
      toast(r.closed ? t('quit_done', { name: a.name, size: fmt(r.freed) }) : t('quit_pending', { name: a.name }));
    } catch (e) {
      toast(t('err_generic', { msg: e.message }));
    }
    S.quitting = null;
    loadMem();
  };
}

/* ─── Apps ────────────────────────────────────────────────────── */

const UNUSED_DAYS = 90;

function appTotal(a) { return a.size + a.leftover_size; }
function isUnused(a) { return !!a.last_used && Date.now() / 1000 - a.last_used > UNUSED_DAYS * 86400; }
function total(list, f) { return list.reduce((s, x) => s + f(x), 0); }
function initial(name) { return (name.trim()[0] || '?').toUpperCase(); }

// "/Users/me/Library/Caches/x" → "~/Library/Caches/x"
function shortPath(p) {
  return String(p || '').replace(/^\/Users\/[^/]+/, '~').replace(/^[A-Za-z]:\\Users\\[^\\]+/, '~');
}

// The Library folder an item sits in: "Application Support", "Caches", …
function placeOf(p) {
  const m = /Library[\\/]([^\\/]+)/.exec(p);
  return m ? m[1] : '';
}

function loadApps(refresh) {
  if (S.appsPolling) return;
  S.appsPolling = true;
  let args = refresh ? { refresh: true } : {};
  const poll = async () => {
    try {
      const r = await api('apps', args);
      args = {};
      if (r.apps) S.apps = r;
      showAppsProgress(r);
      if (r.loading) { setTimeout(poll, 400); return; }
      renderApps();
    } catch (e) {
      $('#appsLoading').hidden = true;
      $('#appsList').innerHTML = `<div class="mem-empty">${esc(t('err_generic', { msg: e.message }))}</div>`;
    }
    S.appsPolling = false;
  };
  poll();
}

function showAppsProgress(r) {
  $('#appsLoading').hidden = !r.loading;
  $('#appsRefresh').disabled = !!r.loading;
  if (!r.loading) return;
  $('#appsBar').style.width = (r.total ? 100 * r.done / r.total : 2) + '%';
  $('#appsLoadingText').textContent = t('apps_loading', { d: num(r.done), n: num(r.total || 0) });
}

function renderApps() {
  const d = S.apps;
  if (!d || !d.apps) return;
  const apps = d.apps;
  $('#appsCount').textContent = num(apps.length);
  const open = apps.filter(a => a.running).length;
  $('#appsRunning').textContent = open ? t('apps_running_n', { n: num(open) }) : '';
  $('#appsSize').textContent = fmt(total(apps, a => a.size));
  $('#appsData').textContent = fmt(total(apps, a => a.leftover_size));
  const unused = apps.filter(isUnused);
  $('#appsUnused').textContent = fmt(total(unused, appTotal));
  $('#appsUnusedN').textContent = t('n_apps', { n: num(unused.length) });
  $$('#appsSort button').forEach(b => b.classList.toggle('on', b.dataset.sort === S.appsSort));

  const q = $('#appsSearch').value.trim().toLowerCase();
  const list = apps.filter(a => !q || a.name.toLowerCase().includes(q) || (a.id || '').toLowerCase().includes(q));
  const when = a => a.last_used || a.installed || 0;
  list.sort({
    size: (a, b) => appTotal(b) - appTotal(a),
    // Never-used (0) first, then the longest unused.
    used: (a, b) => when(a) - when(b) || appTotal(b) - appTotal(a),
    name: (a, b) => a.name.localeCompare(b.name, lang),
  }[S.appsSort] || ((a, b) => appTotal(b) - appTotal(a)));
  const max = Math.max(1, ...apps.map(appTotal));
  const box = $('#appsList');
  box.innerHTML = '';
  if (!list.length) box.append(el('div', 'mem-empty', esc(t(q ? 'apps_none_match' : 'apps_none'))));
  for (const a of list) box.append(appRow(a, max));
  renderOrphans();
}

function appRow(a, max) {
  const row = el('div', 'mem-row app-row' + (a.can_remove ? '' : ' locked'));
  const meta = [];
  if (a.version) meta.push(a.version);
  if (a.last_used) meta.push(t('apps_used', { t: ago(a.last_used) }));
  else if (a.installed) meta.push(t('apps_installed', { t: ago(a.installed) }));
  if (a.leftover_size) meta.push(t('apps_split', { app: fmt(a.size), data: fmt(a.leftover_size) }));
  const tags = [];
  if (a.running) tags.push(`<span class="mem-tag on">${esc(t('apps_open'))}</span>`);
  if (isUnused(a)) tags.push(`<span class="mem-tag idle">${esc(t('apps_unused_tag'))}</span>`);
  const sum = appTotal(a);
  const appShare = sum ? 100 * a.size / sum : 100;
  row.innerHTML = `
    <div class="mem-avatar" style="background:${avatarColor(a.name)}">${esc(initial(a.name))}</div>
    <div style="min-width:0">
      <div class="mem-name" title="${esc(a.path || a.name)}">${esc(a.name)}${tags.join('')}</div>
      <div class="mem-meta">${esc(meta.join(' · '))}</div>
    </div>
    <div class="mem-bar app-bar"><i style="width:${Math.max(1.5, 100 * sum / max)}%"><b style="width:${appShare}%"></b></i></div>
    <div class="mem-size">${fmt(sum)}</div>
    ${a.can_remove ? `<button class="btn">${esc(t('uninstall'))}</button>` : `<div class="mem-lock">${esc(t('apps_self'))}</div>`}`;
  if (a.can_remove) row.onclick = () => askUninstall(a);
  return row;
}

function askUninstall(a) {
  const win = OS === 'windows';
  const items = a.leftovers;
  const sel = new Set(items.filter(l => l.sure).map(l => l.path));
  const notes = [];
  if (win) notes.push(['', t('un_win_flow')]);
  if (a.running) notes.push(['', t('un_running')]);
  if (a.extension) notes.push(['warn', t('un_extension')]);
  const rows = items.map((l, i) => `
    <label class="un-row">
      <span class="check"><input type="checkbox" data-i="${i}" ${sel.has(l.path) ? 'checked' : ''}><span></span></span>
      <div style="min-width:0">
        <div class="un-kind">${esc(t('kind_' + l.kind))}${l.admin ? `<span class="mem-tag">${esc(t('un_admin_tag'))}</span>` : ''}${l.sure ? '' : `<span class="mem-tag idle">${esc(t('un_name_tag'))}</span>`}</div>
        <div class="un-path" title="${esc(l.path)}">&lrm;${esc(shortPath(l.path))}</div>
      </div>
      <span class="un-size">${fmt(l.size)}</span>
    </label>`).join('');
  openModal(`
    <div class="un-head">
      <div class="mem-avatar" style="background:${avatarColor(a.name)}">${esc(initial(a.name))}</div>
      <div style="min-width:0">
        <h3 class="modal-title" id="modalTitle">${esc(t('un_title', { name: a.name }))}</h3>
        <div class="mem-meta">${esc([a.version, shortPath(a.path)].filter(Boolean).join(' · '))}</div>
      </div>
    </div>
    ${notes.map(([cls, text]) => `<div class="un-note ${cls}">${esc(text)}</div>`).join('')}
    <div class="un-list">
      <div class="un-row un-app">
        <span class="check"><input type="checkbox" checked disabled><span></span></span>
        <div style="min-width:0">
          <div class="un-kind">${esc(t('un_app'))}</div>
          <div class="un-path" title="${esc(a.path || '')}">&lrm;${esc(shortPath(a.path || a.name))}</div>
        </div>
        <span class="un-size">${fmt(a.size)}</span>
      </div>
      ${rows}
    </div>
    ${items.length ? '' : `<p class="modal-text">${esc(t('un_nothing_left'))}</p>`}
    ${items.some(l => !l.sure) ? `<p class="modal-text">${esc(t('un_review_note'))}</p>` : ''}
    <div class="un-note warn" id="unData" hidden></div>
    <p class="modal-text" id="unAdmin" hidden></p>
    <div class="un-sum" id="unSum"></div>
    <div class="modal-actions">
      <button class="btn" data-close data-autofocus>${esc(t('cancel'))}</button>
      <button class="btn btn-danger" id="confirmUninstall">${esc(t('uninstall'))}</button>
    </div>`, true);
  const update = () => {
    const chosen = items.filter(l => sel.has(l.path));
    // The install folder is the app itself, already in a.size.
    $('#unSum').innerHTML = t('un_sum', { size: esc(fmt(a.size + total(chosen.filter(l => l.kind !== 'install'), l => l.size))), n: num(chosen.length) });
    // Sandboxed data and support folders can hold chats and documents: say so when it is a lot.
    const data = total(chosen.filter(l => l.kind === 'container' || l.kind === 'support'), l => l.size);
    $('#unData').hidden = data < 100e6;
    $('#unData').textContent = t('un_data_warn', { size: fmt(data) });
    const admin = chosen.filter(l => l.admin).length;
    $('#unAdmin').hidden = !admin;
    $('#unAdmin').textContent = t('un_admin_note', { n: num(admin) });
  };
  $$('#modalBody input[data-i]').forEach(cb => cb.addEventListener('change', () => {
    const l = items[+cb.dataset.i];
    if (cb.checked) sel.add(l.path); else sel.delete(l.path);
    update();
  }));
  update();
  $('#confirmUninstall').onclick = () => runUninstall(a, Array.from(sel));
}

async function runUninstall(a, paths) {
  try {
    await api('uninstall', { key: a ? a.key : null, paths, lang });
  } catch (e) {
    toast(t('err_generic', { msg: e.message }));
    return;
  }
  const win = OS === 'windows';
  // [job step, label]
  const steps = !a ? [['remove', 'step_files']]
    : win ? [['uninstaller', 'step_uninstaller'], ['waiting', 'step_waiting'], ['remove', 'step_files']]
    : [['quit', 'step_quit'], ['remove', 'step_remove']];
  modalLocked = true;
  openModal(`
    <h3 class="modal-title" id="modalTitle">${esc(a ? t('un_running_title', { name: a.name }) : t('orphans_running'))}</h3>
    <ol class="steps">${steps.map(([, k]) => `<li><i></i><span>${esc(t(k))}</span></li>`).join('')}</ol>
    <p class="modal-text" id="unHint" hidden></p>
    <div class="modal-actions" id="unStopRow" hidden><button class="btn" id="unStop">${esc(t('un_stop'))}</button></div>`);
  $('#unStop').onclick = () => api('uninstall_stop').catch(() => {});
  const hints = { quit: 'un_quit_hint', uninstaller: 'un_uninstaller_hint', waiting: 'un_waiting_hint' };
  const poll = async () => {
    let j;
    try { j = (await api('apps', { brief: true })).job; } catch (_) { setTimeout(poll, 600); return; }
    const at = Math.max(0, steps.findIndex(([s]) => s === j.step));
    $$('.steps li').forEach((li, i) => { li.className = i < at ? 'done' : i === at ? 'now' : ''; });
    const hint = hints[j.step] ? t(hints[j.step]) : '';
    $('#unHint').hidden = !hint;
    $('#unHint').textContent = hint;
    $('#unStopRow').hidden = j.step !== 'waiting';
    if (j.running) { setTimeout(poll, 400); return; }
    modalLocked = false;
    finishUninstall(a, j);
  };
  poll();
}

function finishUninstall(a, j) {
  const errs = j.errors || [];
  const close = `<div class="modal-actions"><button class="btn btn-primary" data-close data-autofocus>${esc(t('close'))}</button></div>`;
  if (j.still_open || j.cancelled) {
    const title = j.still_open ? t('un_still_open_title', { name: a ? a.name : '' }) : t('un_cancelled_title');
    openModal(`
      <h3 class="modal-title" id="modalTitle">${esc(title)}</h3>
      <p class="modal-text">${esc(t(j.still_open ? 'un_still_open' : 'un_cancelled'))}</p>${close}`);
  } else {
    const done = !a || j.removed_app;
    openModal(`
      ${done ? `<div class="done-mark">${ICON.check}</div>` : ''}
      <h3 class="modal-title" id="modalTitle">${esc(!done ? t('done_title') : a ? t('un_done_title', { name: a.name }) : t('orphans_done_title'))}</h3>
      ${j.moved + j.freed ? `<div class="modal-figure">${fmt(j.moved + j.freed)}<small>${esc(t('un_figure'))}</small></div>` : ''}
      ${j.moved ? `<p class="modal-text">${esc(t('done_trash_note'))}</p>` : ''}
      ${errs.length ? `<p class="modal-text">${esc(t('done_errors', { n: errs.length }))}</p><div class="err-list">${errs.map(esc).join('<br>')}</div>` : ''}
      ${close}`);
  }
  S.orphanSel.clear();
  loadApps();
}

function renderOrphans() {
  const os = (S.apps && S.apps.orphans) || [];
  $('#orphansCard').hidden = !os.length;
  for (const id of Array.from(S.orphanSel)) if (!os.some(o => o.id === id)) S.orphanSel.delete(id);
  const box = $('#orphansList');
  box.innerHTML = '';
  for (const o of os) {
    const row = el('label', 'mem-row orphan-row');
    const places = Array.from(new Set(o.items.map(i => placeOf(i.path)).filter(Boolean)));
    const meta = [t('orphan_items', { n: num(o.items.length) })];
    if (o.newest) meta.push(t('orphan_changed', { t: ago(o.newest) }));
    row.title = o.items.map(i => shortPath(i.path)).join('\n');
    row.innerHTML = `
      <span class="check"><input type="checkbox" ${S.orphanSel.has(o.id) ? 'checked' : ''}><span></span></span>
      <div style="min-width:0">
        <div class="mem-name">${esc(o.id)}</div>
        <div class="mem-meta">${esc(meta.concat(places).join(' · '))}</div>
      </div>
      <div class="mem-size">${fmt(o.size)}</div>`;
    $('input', row).addEventListener('change', ev => {
      if (ev.target.checked) S.orphanSel.add(o.id); else S.orphanSel.delete(o.id);
      renderOrphanBtn();
    });
    box.append(row);
  }
  renderOrphanBtn();
}

function chosenOrphans() {
  return ((S.apps && S.apps.orphans) || []).filter(o => S.orphanSel.has(o.id));
}

function renderOrphanBtn() {
  const chosen = chosenOrphans();
  const btn = $('#orphansBtn');
  btn.disabled = !chosen.length;
  btn.className = 'btn' + (chosen.length ? ' btn-primary' : '');
  btn.textContent = chosen.length ? t('orphans_btn_n', { size: fmt(total(chosen, o => o.size)) }) : t('orphans_btn');
}

function askOrphans() {
  const chosen = chosenOrphans();
  if (!chosen.length) return;
  const size = fmt(total(chosen, o => o.size));
  openModal(`
    <h3 class="modal-title" id="modalTitle">${esc(t('orphans_confirm', { size }))}</h3>
    <div class="sum-list">${chosen.map(o => `<div><span>${esc(o.id)}</span><b>${fmt(o.size)}</b></div>`).join('')}</div>
    <p class="modal-text">${esc(t('orphans_confirm_d'))}</p>
    <div class="modal-actions">
      <button class="btn" data-close data-autofocus>${esc(t('cancel'))}</button>
      <button class="btn btn-primary" id="confirmOrphans">${esc(t('confirm_move'))}</button>
    </div>`);
  $('#confirmOrphans').onclick = () => runUninstall(null, chosen.flatMap(o => o.items.map(i => i.path)));
}

async function startScan() {
  showScanScreen(true);
  $('#scanError').hidden = true;
  closeDrawer();
  try {
    await api('scan');
    pollScan();
  } catch (e) {
    showScanError(e.message);
  }
}

function showScanError(msg) {
  const box = $('#scanError');
  box.hidden = false;
  box.innerHTML = esc(t('scan_failed', { msg })) + ' <button class="btn btn-sm" id="retryBtn">' + esc(t('retry')) + '</button>';
  $('#retryBtn').onclick = startScan;
}

async function pollScan() {
  let s;
  try {
    s = await api('status');
  } catch (e) {
    showScanError(e.message);
    return;
  }
  $('#scanBytes').textContent = fmt(s.bytes);
  $('#scanFiles').textContent = num(s.files);
  $('#scanTime').textContent = Math.floor(s.elapsed_ms / 1000) + 's';
  // Reads blocked by a pending macOS permission dialog look like a freeze.
  if (s.files !== S.lastFiles) { S.lastFiles = s.files; S.lastMove = Date.now(); }
  const finishing = s.current === '@sysinfo';
  const stalled = s.phase === 'scanning' && !finishing && Date.now() - (S.lastMove || Date.now()) > 8000;
  $('#scanStalled').hidden = !stalled;
  $('#scanPath').textContent = finishing ? t('scan_sysinfo') : s.current ? '\u200e' + s.current + '\u200e' : '\u00a0';
  if (s.phase === 'scanning') {
    setTimeout(pollScan, 200);
  } else if (s.phase === 'ready') {
    S.sel.clear();
    S.firstLoad = true;
    showScanScreen(false);
    await loadAll(0);
  } else if (s.error) {
    showScanError(s.error);
  } else {
    // Cancelled before anything was scanned.
    $('#scanError').hidden = false;
    $('#scanError').innerHTML = '<button class="btn btn-sm" id="retryBtn">' + esc(t('retry')) + '</button>';
    $('#retryBtn').onclick = startScan;
  }
}

/* ─── Loading ─────────────────────────────────────────────────── */

async function loadAll(nodeId) {
  const [overview, recs] = await Promise.all([api('overview'), api('recommendations')]);
  S.overview = overview;
  S.recs = recs;
  const valid = new Set(itemsOf().map(i => i.id));
  for (const id of Array.from(S.sel)) if (!valid.has(id)) S.sel.delete(id);
  if (S.firstLoad) {
    for (const it of itemsOf('safe')) S.sel.add(it.id);
    S.firstLoad = false;
  }
  try {
    S.node = await api('node', { id: nodeId });
  } catch (_) {
    S.node = await api('node', { id: 0 });
  }
  renderAll();
}

async function navigate(id, spot) {
  try {
    S.node = await api('node', { id });
    S.spot = spot == null ? null : spot;
    hideTooltip();
    renderExplorer();
  } catch (e) {
    toast(t('err_generic', { msg: e.message }));
  }
}

/* ─── Render: everything ─────────────────────────────────────── */

function renderAll() {
  renderStatic();
  renderStats();
  renderExplorer();
  renderRecs();
  if (S.drawer) {
    const r = S.recs.find(x => x.id === S.drawer);
    if (r) renderDrawer(r); else closeDrawer();
  }
}

function renderStatic() {
  document.documentElement.lang = lang;
  $$('[data-i18n]').forEach(e => { e.textContent = t(e.dataset.i18n); });
  $$('#langSeg button').forEach(b => b.classList.toggle('on', b.dataset.lang === lang));
  applyTheme();
  $$('#viewSeg button').forEach(b => b.classList.toggle('on', b.dataset.view === S.view));
  $('#appsSearch').placeholder = t('apps_search');
  $('#appsRefresh').title = t('apps_refresh');
  $('#appsRefresh').setAttribute('aria-label', t('apps_refresh'));
  const o = S.overview;
  if (o) {
    $('#brandSub').textContent = t('scanned_in', { files: num(o.files), sec: (o.scan_ms / 1000).toFixed(1) })
      + (o.elevated ? ' · ' + t('as_admin') : '');
    // Windows: offer an administrator rescan whenever anything (system
    // folders included) was unreadable. macOS: Full Disk Access for home.
    const win = OS === 'windows';
    const missing = win ? (o.elevated ? 0 : o.unreadable_all) : o.unreadable;
    const show = missing > 0 && !S.fdaHidden;
    $('#fdaBanner').hidden = !show;
    if (show) $('#fdaText').textContent = t(win ? 'admin_text' : 'fda_text', { n: num(missing) });
    $('#fdaBtn').textContent = t(win ? 'admin_button' : 'fda_button');
  }
}

function renderStats() {
  const o = S.overview;
  if (!o) return;
  const sel = selectionTotals();
  const safe = S.recs.filter(r => r.level === 'safe');
  const review = S.recs.filter(r => r.level === 'review');
  const used = Math.max(0, o.disk_total - o.disk_free);
  const outside = Math.max(0, used - o.scanned);

  $('#statFree').textContent = fmt(o.disk_free + sel.size);
  $('#statDelta').textContent = sel.size ? '+' + fmt(sel.size) : '';
  $('#statSafe').textContent = fmt(safe.reduce((a, r) => a + r.size, 0));
  $('#statSafeN').textContent = t('n_groups', { n: safe.length });
  $('#statReview').textContent = fmt(review.reduce((a, r) => a + r.size, 0));
  $('#statReviewN').textContent = t('n_groups', { n: review.length });
  $('#statOther').textContent = fmt(outside);

  // Disk bar: home by category, then everything outside home, then free.
  const bar = $('#diskbar');
  const total = o.disk_total || 1;
  const pct = v => (100 * v / total).toFixed(3) + '%';
  let html = '';
  for (const c of CATS) {
    const v = o.cats[c] || 0;
    if (v > 0) html += `<i style="width:${pct(v)};background:var(--c-${c}-ac)" title="${esc(t('cat_' + c))} ${fmt(v)}"></i>`;
  }
  html += `<i style="width:${pct(outside)};background:var(--c-other)" title="${esc(t('lg_system'))} ${fmt(outside)}"></i>`;
  bar.innerHTML = html;
  bar.style.position = 'relative';
  if (sel.size > 0) {
    const ov = el('i', 'seg-selected');
    ov.style.cssText = `position:absolute;top:0;bottom:0;right:${pct(o.disk_free)};width:${pct(sel.size)}`;
    ov.title = `${t('lg_selected')} ${fmt(sel.size)}`;
    bar.append(ov);
  }
  let lg = '';
  for (const c of CATS) {
    const v = o.cats[c] || 0;
    if (v > 0) lg += `<span><i class="swatch" style="background:var(--c-${c}-ac)"></i>${esc(t('cat_' + c))} <b>${fmt(v)}</b></span>`;
  }
  lg += `<span><i class="swatch" style="background:var(--c-other)"></i>${esc(t('lg_system'))} <b>${fmt(outside)}</b></span>`;
  lg += `<span><i class="swatch" style="background:var(--c-free);box-shadow:inset 0 0 0 1px var(--line-strong)"></i>${esc(t('lg_free'))} <b>${fmt(o.disk_free)}</b></span>`;
  if (sel.size > 0) lg += `<span><i class="swatch" style="background:repeating-linear-gradient(135deg,var(--safe) 0 2px,var(--safe-bg) 2px 4px)"></i>${esc(t('lg_selected'))} <b>${fmt(sel.size)}</b></span>`;
  $('#diskbarLegend').innerHTML = lg;
}

/* ─── Treemap ─────────────────────────────────────────────────── */

function worst(row, side) {
  let s = 0, max = 0, min = Infinity;
  for (const v of row) { s += v; if (v > max) max = v; if (v < min) min = v; }
  const s2 = s * s, side2 = side * side;
  return Math.max(side2 * max / s2, s2 / (side2 * min));
}

/** Squarified treemap layout: values (sorted big→small) into a rectangle. */
function squarify(values, rect) {
  const total = values.reduce((a, b) => a + b, 0);
  const out = [];
  if (total <= 0 || rect.w <= 0 || rect.h <= 0) return values.map(() => ({ x: 0, y: 0, w: 0, h: 0 }));
  const scale = rect.w * rect.h / total;
  const areas = values.map(v => v * scale);
  const r = { ...rect };
  let row = [];
  const flush = () => {
    const s = row.reduce((a, b) => a + b, 0);
    if (r.w >= r.h) {
      const cw = s / r.h;
      let y = r.y;
      for (const a of row) { const h = a / cw; out.push({ x: r.x, y, w: cw, h }); y += h; }
      r.x += cw; r.w -= cw;
    } else {
      const rh = s / r.w;
      let x = r.x;
      for (const a of row) { const w = a / rh; out.push({ x, y: r.y, w, h: rh }); x += w; }
      r.y += rh; r.h -= rh;
    }
    row = [];
  };
  let i = 0;
  while (i < areas.length) {
    const side = Math.min(r.w, r.h);
    if (row.length === 0 || worst(row.concat(areas[i]), side) <= worst(row, side)) {
      row.push(areas[i]);
      i++;
    } else {
      flush();
    }
  }
  if (row.length) flush();
  return out;
}

function inset(r, g) {
  return { x: r.x + g, y: r.y + g, w: Math.max(0, r.w - 2 * g), h: Math.max(0, r.h - 2 * g) };
}

function tileName(c) {
  if (c.kind === 'files') return t('small_files', { n: num(c.count) });
  if (c.kind === 'more') return t('more_items', { n: num(c.count) });
  if (c.kind === 'system') return t('macos_tile');
  if (c.name === '~') return '~ ' + t('home');
  return c.name;
}

function makeTile(c, r, inner) {
  const tile = el('div', 'tile' + (inner ? ' inner' : ''));
  tile.style.cssText = `left:${r.x}px;top:${r.y}px;width:${r.w}px;height:${r.h}px;`
    + `--tb:var(--c-${c.cat});--ti:var(--c-${c.cat}-in);--tl:var(--c-${c.cat}-line);--ta:var(--c-${c.cat}-ac)`;
  if (c.kind === 'files' || c.kind === 'more') tile.classList.add('agg');
  if (c.kind === 'system') tile.classList.add('sysrest');
  if (c.status) tile.classList.add(c.status);
  if (c.contains) tile.classList.add('holds');
  if (c.id != null) tile.dataset.id = c.id;
  if (c.rec) tile.dataset.rec = c.rec;
  tile._data = c;
  return tile;
}

/** Thin bar along the bottom: how much of this folder can be reclaimed. */
function reclaimBar(tile, c, r, inner, hasInner) {
  const [safe, review] = c.contains || [0, 0];
  if (!safe && !review) return;
  const bar = el('div', 't-reclaim');
  const pct = v => Math.min(100, 100 * v / c.size) + '%';
  bar.innerHTML = `<i class="rs" style="width:${pct(safe)}"></i><i class="rr" style="width:${pct(review)}"></i>`;
  tile.append(bar);
  if (!inner && r.w > 120 && r.h > 50) {
    const txt = el('div', 't-reclaim-text' + (hasInner ? ' top' : ''), esc(fmt(safe + review)));
    txt.title = t('lg_selected');
    tile.append(txt);
  }
}

function label(tile, c, r, inner, hasInner) {
  reclaimBar(tile, c, r, inner, hasInner);
  const name = tileName(c);
  if (inner) {
    if (r.w > 52 && r.h > 19) tile.append(el('div', 't-name', esc(name)));
    if (r.w > 58 && r.h > 46) tile.append(el('div', 't-big', fmt(c.size)));
    return;
  }
  if (r.w > 44 && r.h > 22) tile.append(el('div', 't-name', esc(name)));
  if (hasInner) {
    if (r.w > 150) { tile.append(el('div', 't-size', fmt(c.size))); tile.classList.add('has-size-top'); }
  } else if (r.w > 60 && r.h > 48) {
    tile.append(el('div', 't-big', fmt(c.size)));
    if (r.w > 170 && r.h > 120) tile.classList.add('huge');
  }
}

function renderTreemap() {
  const box = $('#treemap');
  box.className = 'treemap mode-' + S.view;
  box.innerHTML = '';
  const n = S.node;
  if (!n) return;
  const W = box.clientWidth, H = box.clientHeight;
  let kids = (n.children || []).filter(c => c.size > 0);
  if (n.kind === 'file') kids = [{ ...n, children: [] }];
  if (!kids.length) {
    box.append(el('div', 'treemap-empty', esc(t('empty_folder'))));
    return;
  }
  const rects = squarify(kids.map(k => k.size), { x: 0, y: 0, w: W, h: H });
  kids.forEach((c, i) => {
    const r = inset(rects[i], 2.5);
    if (r.w < 3 || r.h < 3) return;
    const tile = makeTile(c, r, false);
    const inner = (c.children || []).filter(k => k.size > 0);
    const area = { x: 5, y: 27, w: r.w - 12, h: r.h - 34 };
    const canNest = inner.length > 0 && !(inner.length === 1 && inner[0].kind !== 'dir') && area.w > 30 && area.h > 26;
    label(tile, c, r, false, canNest);
    if (canNest) {
      const sub = squarify(inner.map(k => k.size), area);
      inner.forEach((k, j) => {
        const rr = inset(sub[j], 1.5);
        if (rr.w < 3 || rr.h < 3) return;
        const it = makeTile(k, rr, true);
        label(it, k, rr, true, false);
        tile.append(it);
      });
    }
    box.append(tile);
  });
  if (S.spot != null) {
    const lit = box.querySelector(`.tile[data-id="${S.spot}"]`);
    if (lit) {
      box.classList.add('spot');
      lit.classList.add('lit');
      setTimeout(() => { box.classList.remove('spot'); lit.classList.remove('lit'); S.spot = null; }, 2200);
    }
  }
}

function renderCrumbs() {
  const n = S.node;
  const nav = $('#crumbs');
  nav.innerHTML = '';
  (n.crumbs || []).forEach((c, i) => {
    if (i) nav.append(el('span', 'sep', '/'));
    const label = i === 0 && n.whole_mac ? t('this_mac') : c.name === '~' ? '~ ' + t('home') : c.name;
    const b = el('button', null, esc(label));
    b.onclick = () => navigate(c.id);
    nav.append(b);
  });
  $('#crumbSize').textContent = fmt(n.size);
  $('#upBtn').disabled = (n.crumbs || []).length < 2;
}

function renderLegend() {
  let html = CATS.map(c => `<span><i class="swatch" style="background:var(--c-${c}-ac)"></i>${esc(t('cat_' + c))}</span>`).join('');
  html += `<span><i class="hatch-swatch hs-safe"></i>${esc(t('lg_safe'))}</span>`;
  html += `<span><i class="hatch-swatch hs-review"></i>${esc(t('lg_review'))}</span>`;
  $('#legend').innerHTML = html;
}

function renderExplorer() {
  if (!S.node) return;
  renderCrumbs();
  renderTreemap();
  renderLegend();
}

/* Tooltip */

function showTooltip(tile, ev) {
  const c = tile._data;
  const tip = $('#tooltip');
  const parentTile = tile.classList.contains('inner') ? tile.parentElement._data : null;
  const name = tileName(c);
  let path = '';
  if (c.kind === 'dir' || c.kind === 'file') {
    const join = (a, b) => (b.startsWith('/') || b.startsWith('~')) ? b : (a === '/' ? '/' + b : a + '/' + b);
    path = join(parentTile ? join(S.node.path, parentTile.name) : S.node.path, c.name);
  }
  const share = S.node.size ? Math.round(100 * c.size / S.node.size) : 0;
  let html = `<div class="tt-name">${esc(path || name)}</div>`;
  html += `<div class="tt-row"><span>${esc(fmt(c.size))}</span><b>${share}% ${esc(t('of_folder'))}</b></div>`;
  if (c.files > 1) html += `<div class="tt-row"><span>${esc(t('files_count', { n: num(c.files) }))}</span></div>`;
  if (c.contains && !c.status) {
    const [sf, rv] = c.contains;
    if (sf) html += `<div class="tt-tag"><i class="dot dot-safe"></i>${esc(t('lg_safe'))} ${esc(fmt(sf))}</div>`;
    if (rv) html += `<div class="tt-tag"><i class="dot dot-review"></i>${esc(t('lg_review'))} ${esc(fmt(rv))}</div>`;
  }
  if (c.status) {
    const r = S.recs.find(x => x.id === c.rec);
    html += `<div class="tt-tag"><i class="dot dot-${c.status}"></i>${esc(r ? (lang === 'th' ? r.title_th : r.title_en) : '')}</div>`;
  }
  if (c.kind === 'system') html += `<div class="tt-hint">${esc(t('macos_tip'))}</div>`;
  if (c.kind === 'dir') html += `<div class="tt-hint">${esc(t('tt_click'))}</div>`;
  else if (c.kind === 'file') html += `<div class="tt-hint">${esc(t('tt_right'))}</div>`;
  tip.innerHTML = html;
  tip.hidden = false;
  moveTooltip(ev);
}

function moveTooltip(ev) {
  const tip = $('#tooltip');
  if (tip.hidden) return;
  const pad = 14;
  let x = ev.clientX + pad, y = ev.clientY + pad;
  const w = tip.offsetWidth, h = tip.offsetHeight;
  if (x + w > innerWidth - 8) x = ev.clientX - w - pad;
  if (y + h > innerHeight - 8) y = ev.clientY - h - pad;
  tip.style.left = x + 'px';
  tip.style.top = y + 'px';
}

function hideTooltip() { $('#tooltip').hidden = true; }

/* Context menu */

function openMenu(ev, entries) {
  const m = $('#menu');
  m.innerHTML = '';
  for (const e of entries) {
    const b = el('button', null, e.icon + '<span>' + esc(e.label) + '</span>');
    b.onclick = () => { closeMenu(); e.run(); };
    m.append(b);
  }
  m.hidden = false;
  const w = m.offsetWidth, h = m.offsetHeight;
  m.style.left = Math.min(ev.clientX, innerWidth - w - 8) + 'px';
  m.style.top = Math.min(ev.clientY, innerHeight - h - 8) + 'px';
}
function closeMenu() { $('#menu').hidden = true; }

function tileFromEvent(ev) {
  const tile = ev.target.closest('.tile');
  return tile && tile._data && tile._data.id != null ? tile : null;
}

function bindTreemap() {
  const box = $('#treemap');
  box.addEventListener('click', ev => {
    const tile = tileFromEvent(ev);
    if (!tile) return;
    const c = tile._data;
    if (c.kind === 'dir') navigate(c.id);
  });
  box.addEventListener('contextmenu', ev => {
    const tile = tileFromEvent(ev);
    if (!tile) return;
    ev.preventDefault();
    hideTooltip();
    const c = tile._data;
    const entries = [];
    if (c.kind === 'dir') entries.push({ icon: ICON.open, label: t('open_folder'), run: () => navigate(c.id) });
    entries.push({ icon: ICON.finder, label: t('reveal'), run: () => reveal(c.id) });
    if (c.rec) entries.push({ icon: ICON.tip, label: t('show_rec'), run: () => openDrawer(c.rec) });
    openMenu(ev, entries);
  });
  box.addEventListener('mouseover', ev => {
    const tile = ev.target.closest('.tile');
    if (tile && tile._data) showTooltip(tile, ev); else hideTooltip();
  });
  box.addEventListener('mousemove', moveTooltip);
  box.addEventListener('mouseleave', hideTooltip);
  let raf = 0;
  new ResizeObserver(() => {
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(() => { if (S.node && !$('#app').hidden) renderTreemap(); });
  }).observe(box);
}

async function reveal(id) {
  try { await api('reveal', { id }); } catch (e) { toast(t('err_generic', { msg: e.message })); }
}

/* ─── Recommendations ─────────────────────────────────────────── */

function recTitle(r) { return lang === 'th' ? r.title_th : r.title_en; }
function recDesc(r) { return lang === 'th' ? r.desc_th : r.desc_en; }

function setCheckbox(input, n, total) {
  input.checked = n > 0 && n === total;
  input.indeterminate = n > 0 && n < total;
}

function renderRecs() {
  const list = $('#recsList');
  list.innerHTML = '';
  if (!S.recs.length) {
    list.append(el('div', 'recs-empty', `<b>${esc(t('recs_empty_t'))}</b>${esc(t('recs_empty'))}`));
  }
  for (const level of ['safe', 'review']) {
    const recs = S.recs.filter(r => r.level === level);
    if (!recs.length) continue;
    const total = recs.reduce((a, r) => a + r.size, 0);
    list.append(el('div', 'recs-group',
      `<span class="pill pill-${level}">${esc(t('group_' + level))}</span><span class="mono">${fmt(total)}</span>`));
    for (const r of recs) {
      const row = el('div', 'rec');
      row.dataset.rec = r.id;
      const s = recSelected(r);
      row.innerHTML = `
        <label class="check" title="${esc(t('select_all'))}"><input type="checkbox"><span></span></label>
        <div class="rec-main"><div class="rec-title">${esc(recTitle(r))}</div><div class="rec-hint">${esc(r.hint)}</div></div>
        <div class="rec-size">${fmt(r.size)}${r.items.length > 1 ? `<small>${esc(t('n_items', { n: num(r.items.length) }))}</small>` : ''}</div>
        <div class="rec-go">${ICON.chevron}</div>`;
      const cb = $('input', row);
      setCheckbox(cb, s.n, r.items.length);
      cb.addEventListener('change', () => {
        const on = s.n < r.items.length;
        for (const it of r.items) on ? S.sel.add(it.id) : S.sel.delete(it.id);
        selectionChanged();
      });
      $('label', row).addEventListener('click', ev => ev.stopPropagation());
      row.addEventListener('click', () => openDrawer(r.id));
      row.addEventListener('mouseenter', () => spotlight(r.id));
      row.addEventListener('mouseleave', () => spotlight(null));
      list.append(row);
    }
  }
  renderFoot();
}

function spotlight(recId) {
  const box = $('#treemap');
  $$('.tile.lit', box).forEach(x => x.classList.remove('lit'));
  if (!recId) { box.classList.remove('spot'); return; }
  const tiles = $$(`.tile[data-rec="${recId}"]`, box);
  if (!tiles.length) { box.classList.remove('spot'); return; }
  tiles.forEach(x => x.classList.add('lit'));
  box.classList.add('spot');
}

function renderFoot() {
  const sel = selectionTotals();
  $('#selSize').textContent = sel.size ? fmt(sel.size) : '';
  $('#selCount').textContent = sel.size
    ? t('selected') + ' ' + t('sel_summary', { r: sel.recs, i: num(sel.items) })
    : t('sel_none');
  const allSafe = itemsOf('safe');
  const allSafeOn = allSafe.length > 0 && allSafe.every(i => S.sel.has(i.id)) && sel.items === allSafe.length;
  const sb = $('#selectSafeBtn');
  sb.textContent = allSafeOn ? t('clear_selection') : t('select_safe');
  sb.dataset.mode = allSafeOn ? 'clear' : 'safe';
  $('#scriptBtn').disabled = !sel.items;
  $('#cleanBtn').disabled = !sel.items;
}

function selectionChanged() {
  renderRecs();
  renderStats();
  if (S.drawer) {
    const r = S.recs.find(x => x.id === S.drawer);
    if (r) renderDrawer(r);
  }
}

/* Drawer */

function openDrawer(recId) {
  const r = S.recs.find(x => x.id === recId);
  if (!r) return;
  S.drawer = recId;
  renderDrawer(r);
  const d = $('#drawer');
  d.classList.add('open');
  d.setAttribute('aria-hidden', 'false');
  $('#drawerItems').scrollTop = 0;
}

const VIRTUAL = 0xF0000000;

/** Plain-language notes for items that are not removed the usual way. */
function actionNotes(items) {
  const kinds = new Set(items.map(it => it.action && it.action.kind));
  const out = [];
  if (kinds.has('admin') || kinds.has('admincommand')) out.push(t('note_admin'));
  if (kinds.has('finder')) out.push(t('note_finder'));
  if (kinds.has('command')) out.push(t('note_command'));
  return out;
}

function closeDrawer() {
  S.drawer = null;
  const d = $('#drawer');
  d.classList.remove('open');
  d.setAttribute('aria-hidden', 'true');
}

function renderDrawer(r) {
  $('#drawerPill').className = 'pill pill-' + r.level;
  $('#drawerPill').textContent = t('group_' + r.level);
  $('#drawerTitle').textContent = recTitle(r);
  $('#drawerDesc').textContent = recDesc(r);
  $('#drawerCmd').hidden = !r.command;
  $('#drawerCmdText').textContent = r.command || '';
  const notes = actionNotes(r.items);
  $('#drawerNote').hidden = !notes.length;
  $('#drawerNote').innerHTML = notes.length ? ICON.tip + '<span>' + notes.map(esc).join(' ') + '</span>' : '';
  const s = recSelected(r);
  setCheckbox($('#drawerAll'), s.n, r.items.length);
  $('#drawerCount').textContent = t('n_items', { n: num(r.items.length) });
  $('#drawerTotal').textContent = s.n ? `${fmt(s.size)} / ${fmt(r.size)}` : fmt(r.size);

  const box = $('#drawerItems');
  const scroll = box.scrollTop;
  box.innerHTML = '';
  for (const it of r.items) {
    const row = el('div', 'item');
    const meta = [];
    if (it.sims != null) meta.push(it.sims ? t('used_by_sims', { n: it.sims }) : t('no_sims'));
    if (it.last_used != null) meta.push(it.last_used ? t('last_opened', { when: ago(it.last_used) }) : t('never_opened'));
    else {
      if (it.files > 1) meta.push(t('files_count', { n: num(it.files) }));
      if (it.newest) meta.push(t('modified', { when: ago(it.newest) }));
    }
    const title = it.label || it.path.split('/').filter(Boolean).slice(-2).join('/');
    row.innerHTML = `
      <label class="check"><input type="checkbox" ${S.sel.has(it.id) ? 'checked' : ''}><span></span></label>
      <div class="item-main">
        <div class="item-label">${esc(title)}</div>
        <div class="item-path" title="${esc(it.path)}">‎${esc(it.path)}‎</div>
        <div class="item-meta">${esc(meta.join(' · '))}</div>
      </div>
      <div class="item-side">
        <span class="item-size">${fmt(it.size)}</span>
        ${it.id < VIRTUAL ? `<button class="icon-btn" data-act="map" title="${esc(t('show_in_map'))}" aria-label="${esc(t('show_in_map'))}">${ICON.map}</button>` : ''}
        <button class="icon-btn" data-act="reveal" title="${esc(t('reveal'))}" aria-label="${esc(t('reveal'))}">${ICON.finder}</button>
      </div>`;
    $('input', row).addEventListener('change', ev => {
      ev.target.checked ? S.sel.add(it.id) : S.sel.delete(it.id);
      selectionChanged();
    });
    const mapBtn = $('[data-act="map"]', row);
    if (mapBtn) mapBtn.onclick = () => showInMap(it.id);
    $('[data-act="reveal"]', row).onclick = () => reveal(it.id);
    box.append(row);
  }
  box.scrollTop = scroll;
}

async function showInMap(id) {
  try {
    const n = await api('node', { id });
    const crumbs = n.crumbs || [];
    const parent = crumbs.length > 1 ? crumbs[crumbs.length - 2].id : 0;
    await navigate(parent, id);
  } catch (e) {
    toast(t('err_generic', { msg: e.message }));
  }
}

/* ─── Modals ──────────────────────────────────────────────────── */

let modalLocked = false;

function openModal(html, wide) {
  const wrap = $('#modal');
  $('.modal', wrap).classList.toggle('wide', !!wide);
  $('#modalBody').innerHTML = html;
  wrap.hidden = false;
  const focus = $('#modalBody [data-autofocus]') || $('#modalBody button');
  if (focus) focus.focus();
}

function closeModal() {
  if (modalLocked) return;
  $('#modal').hidden = true;
}

function selectedIds() { return Array.from(S.sel); }

async function showScript() {
  let script;
  try {
    script = (await api('script', { items: selectedIds(), lang })).script;
  } catch (e) {
    toast(t('err_generic', { msg: e.message }));
    return;
  }
  const pretty = script.split('\n').map(line => {
    if (line.startsWith('#')) return `<span class="c">${esc(line)}</span>`;
    const m = line.match(/^(rm -rf --)(.*?)(\s+# .*)?$/);
    if (!m) return esc(line);
    return `<span class="k">${esc(m[1])}</span>${esc(m[2])}${m[3] ? `<span class="c">${esc(m[3])}</span>` : ''}`;
  }).join('\n');
  openModal(`
    <h3 class="modal-title" id="modalTitle">${esc(t('script_title'))}</h3>
    <p class="modal-text">${esc(t('script_text'))}</p>
    <pre class="script-box">${pretty}</pre>
    <div class="modal-actions">
      <button class="btn" data-close>${esc(t('close'))}</button>
      <button class="btn btn-primary" id="copyScript" data-autofocus>${esc(t('copy'))}</button>
    </div>`, true);
  $('#copyScript').onclick = async () => {
    await copyText(script);
    toast(t('copied'));
  };
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch (_) {
    const ta = el('textarea');
    ta.value = text;
    ta.style.cssText = 'position:fixed;opacity:0';
    document.body.append(ta);
    ta.select();
    document.execCommand('copy');
    ta.remove();
  }
}

function showClean() {
  const sel = selectionTotals();
  if (!sel.items) return;
  const chosen = S.recs.map(r => ({ r, s: recSelected(r) })).filter(x => x.s.n);
  const reviewN = chosen.filter(x => x.r.level === 'review').length;
  const rows = chosen.map(x =>
    `<div><span><i class="dot dot-${x.r.level}"></i>${esc(recTitle(x.r))}</span><b>${fmt(x.s.size)}</b></div>`).join('');
  openModal(`
    <h3 class="modal-title" id="modalTitle">${esc(t('clean_title', { size: fmt(sel.size) }))}</h3>
    <div class="sum-list">${rows}</div>
    ${reviewN ? `<p class="modal-text" style="color:var(--review)">${esc(t('review_warn', { n: reviewN }))}</p>` : ''}
    ${actionNotes(S.recs.flatMap(r => r.items.filter(it => S.sel.has(it.id)))).map(n => `<p class="modal-text">• ${esc(n)}</p>`).join('')}
    <div>
      <p class="modal-text" style="margin-bottom:8px">${esc(t('clean_how'))}</p>
      <div class="choice">
        <label><input type="radio" name="mode" value="trash" checked><div><b>${esc(t('mode_trash'))}</b><span>${esc(t('mode_trash_d'))}</span></div></label>
        <label><input type="radio" name="mode" value="delete"><div><b>${esc(t('mode_delete'))}</b><span>${esc(t('mode_delete_d'))}</span></div></label>
      </div>
    </div>
    <div class="modal-actions">
      <button class="btn" data-close data-autofocus>${esc(t('cancel'))}</button>
      <button class="btn btn-primary" id="confirmClean">${esc(t('confirm_trash'))}</button>
    </div>`);
  const btn = $('#confirmClean');
  $$('input[name="mode"]').forEach(r => r.addEventListener('change', () => {
    const del = $('input[name="mode"]:checked').value === 'delete';
    btn.textContent = t(del ? 'confirm_delete' : 'confirm_trash');
    btn.className = 'btn ' + (del ? 'btn-danger' : 'btn-primary');
  }));
  btn.onclick = () => runClean($('input[name="mode"]:checked').value);
}

async function runClean(mode) {
  try {
    await api('clean', { items: selectedIds(), mode, lang });
  } catch (e) {
    toast(t('err_generic', { msg: e.message }));
    return;
  }
  modalLocked = true;
  openModal(`
    <h3 class="modal-title" id="modalTitle">${esc(t('cleaning'))}</h3>
    <div class="progress"><i id="cleanBar"></i></div>
    <div class="modal-text" id="cleanCount"></div>
    <div class="progress-path" id="cleanPath">&nbsp;</div>`);
  const poll = async () => {
    let s;
    try { s = (await api('status')).clean; } catch (_) { setTimeout(poll, 500); return; }
    $('#cleanBar').style.width = (s.total ? 100 * s.done / s.total : 0) + '%';
    $('#cleanCount').textContent = t('cleaning_n', { d: num(s.done), n: num(s.total) });
    $('#cleanPath').textContent = s.current ? '‎' + s.current : ' ';
    if (s.running) { setTimeout(poll, 250); return; }
    modalLocked = false;
    finishClean(s);
  };
  poll();
}

async function finishClean(s) {
  const trash = s.freed === 0 && s.moved > 0;
  const amount = trash ? s.moved : s.freed;
  const errs = s.errors || [];
  openModal(`
    <div class="done-mark">${ICON.check}</div>
    <h3 class="modal-title" id="modalTitle">${esc(t('done_title'))}</h3>
    <div class="modal-figure">${fmt(amount)}<small>${esc(t(trash ? 'done_moved' : 'done_freed'))}</small></div>
    ${!trash && s.moved ? `<p class="modal-text">${esc(t('done_also_moved', { size: fmt(s.moved) }))}</p>` : ''}
    ${s.moved ? `<p class="modal-text">${esc(t('done_trash_note'))}</p>` : ''}
    ${errs.length ? `<p class="modal-text">${esc(t('done_errors', { n: errs.length }))}</p><div class="err-list">${errs.map(esc).join('<br>')}</div>` : ''}
    <div class="modal-actions"><button class="btn btn-primary" data-close data-autofocus>${esc(t('close'))}</button></div>`);
  await loadAll(S.node ? S.node.id : 0);
}

/* ─── Wiring ──────────────────────────────────────────────────── */

function bind() {
  $$('#langSeg button').forEach(b => b.onclick = () => {
    lang = b.dataset.lang;
    keep('puri-lang', lang);
    renderStatic();
    if (S.overview) renderAll();
    if (S.mem) renderMem();
    if (S.apps) renderApps();
  });
  $('#appsSearch').addEventListener('input', renderApps);
  $$('#appsSort button').forEach(b => b.onclick = () => {
    S.appsSort = b.dataset.sort;
    keep('puri-apps-sort', S.appsSort);
    renderApps();
  });
  $('#appsRefresh').onclick = () => loadApps(true);
  $('#orphansBtn').onclick = askOrphans;
  $$('#viewSeg button').forEach(b => b.onclick = () => {
    S.view = b.dataset.view;
    keep('puri-view', S.view);
    renderStatic();
    renderTreemap();
  });
  $('#rescanBtn').onclick = startScan;
  $$('#tabSeg button').forEach(b => b.onclick = () => {
    S.tab = b.dataset.tab;
    keep('puri-tab', S.tab);
    applyTab();
  });
  $('#themeBtn').onclick = () => {
    const next = currentTheme() === 'dark' ? 'light' : 'dark';
    keep('puri-theme', next);
    applyTheme(next);
  };
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => applyTheme());
  $('#cancelScanBtn').onclick = () => api('cancel').catch(() => {});
  $('#upBtn').onclick = goUp;
  $('#selectSafeBtn').onclick = () => {
    if ($('#selectSafeBtn').dataset.mode === 'clear') S.sel.clear();
    else { S.sel.clear(); for (const it of itemsOf('safe')) S.sel.add(it.id); }
    selectionChanged();
  };
  $('#scriptBtn').onclick = showScript;
  $('#cleanBtn').onclick = showClean;
  $('#drawerBack').onclick = closeDrawer;
  $('#drawerAll').addEventListener('change', () => {
    const r = S.recs.find(x => x.id === S.drawer);
    if (!r) return;
    const s = recSelected(r);
    const on = s.n < r.items.length;
    for (const it of r.items) on ? S.sel.add(it.id) : S.sel.delete(it.id);
    selectionChanged();
  });
  $('#fdaBtn').onclick = async () => {
    if (OS !== 'windows') { api('open_fda').catch(() => {}); return; }
    try {
      await api('relaunch_admin');
      // This copy quits; the elevated one opens its own window.
      openModal(`<h3 class="modal-title" id="modalTitle">${esc(t('admin_relaunching'))}</h3><p class="modal-text">${esc(t('admin_relaunching_d'))}</p>`);
    } catch (e) {
      toast(e.message === 'cancelled' ? t('admin_cancelled') : t('err_generic', { msg: e.message }));
    }
  };
  $('#fdaClose').onclick = () => { S.fdaHidden = true; $('#fdaBanner').hidden = true; };
  $('#modal').addEventListener('click', ev => {
    if (ev.target.id === 'modal' || ev.target.closest('[data-close]')) closeModal();
  });
  document.addEventListener('click', ev => { if (!ev.target.closest('#menu')) closeMenu(); });
  document.addEventListener('keydown', ev => {
    if (ev.key === 'Escape') {
      if (!$('#menu').hidden) closeMenu();
      else if (!$('#modal').hidden) closeModal();
      else if (S.drawer) closeDrawer();
      return;
    }
    const typing = /INPUT|TEXTAREA/.test(document.activeElement.tagName);
    if (ev.key === 'Backspace' && !typing && $('#modal').hidden && !$('#app').hidden) {
      ev.preventDefault();
      goUp();
    }
  });
  window.addEventListener('blur', () => { hideTooltip(); closeMenu(); });
  bindTreemap();
}

function goUp() {
  const c = (S.node && S.node.crumbs) || [];
  if (c.length > 1) navigate(c[c.length - 2].id);
}

async function boot() {
  renderStatic();
  bind();
  try {
    const s = await api('status');
    if (s.phase === 'ready' || s.phase === 'cleaning') {
      showScanScreen(false);
      await loadAll(0);
    } else if (s.phase === 'scanning') {
      showScanScreen(true);
      pollScan();
    } else {
      startScan();
    }
  } catch (e) {
    showScanScreen(true);
    showScanError(e.message);
  }
}

boot();

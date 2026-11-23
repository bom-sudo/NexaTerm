/**
 * NexaTerm Website Bilingual Localization (TH / EN)
 */

const translations = {
  th: {
    page_title: "NexaTerm - โปรแกรมเทอร์มินัล SSH และ SFTP ฟรีสำหรับ Windows",
    page_desc: "NexaTerm รวมเทอร์มินัล SSH, SFTP, SSH tunnel, การสั่งงานหลายเซิร์ฟเวอร์พร้อมกัน และเครื่องมือเครือข่ายไว้ในโปรแกรมเดียวสำหรับ Windows 10/11 ฟรีและโอเพนซอร์ส (MIT) ไม่ต้องสมัครบัญชี ไม่ส่งข้อมูลการใช้งานออกจากเครื่อง",
    
    // Navigation
    nav_brand: "NexaTerm",
    nav_features: "ความสามารถ",
    nav_guide: "วิธีใช้งาน",
    nav_privacy: "ความปลอดภัย",
    nav_download: "ดาวน์โหลด",
    nav_faq: "คำถามที่พบบ่อย",
    nav_download_btn: "ดาวน์โหลด",

    // Hero
    hero_eyebrow: 'NexaTerm 1.0.0 <span>·</span> ฟรีและโอเพนซอร์ส (MIT) <span>·</span> Windows 10 และ 11',
    hero_title: 'หนึ่งพื้นที่ทำงาน<br><span class="grad">ครบทุกการเชื่อมต่อระยะไกล</span>',
    hero_lead: "เทอร์มินัล, SSH, SFTP, SSH tunnel และเครื่องมือตรวจสอบเครือข่ายสำหรับผู้ดูแลระบบ รวมอยู่ในโปรแกรมเดียวบน Windows ไม่ต้องสมัครบัญชี ไม่มีรุ่นเสียเงิน และไม่ส่งข้อมูลการใช้งานออกจากเครื่อง",
    hero_cta_download: "ดาวน์โหลดสำหรับ Windows",
    hero_cta_guide: "ดูวิธีใช้งาน",
    hero_note: "ติดตั้งแบบรายผู้ใช้ ไม่ต้องใช้สิทธิ์ผู้ดูแลระบบ หรือใช้รุ่นพกพาที่ไม่ต้องติดตั้งเลย",
    hero_shot_caption: "PowerShell บนเครื่องและ SSH แสดงคู่กันในแท็บเดียว รูปทั้งหมดในหน้านี้ถ่ายจากโปรแกรมจริง ส่วน SSH และ SFTP เชื่อมต่อกับเซิร์ฟเวอร์ทดสอบที่มาพร้อมซอร์สโค้ด จึงเห็นที่อยู่เป็น 127.0.0.1",

    // Facts
    fact1_title: "ใช้ได้ฟรีทุกฟังก์ชัน",
    fact1_desc: "สัญญาอนุญาต MIT ไม่มีรุ่นเสียเงิน ไม่มีคีย์ลิขสิทธิ์ และไม่จำกัดจำนวน session หรือ tunnel",
    fact2_title: "ไม่เก็บข้อมูลการใช้งาน",
    fact2_desc: "ไม่มี telemetry และไม่เชื่อมต่ออินเทอร์เน็ตเองตอนเปิดโปรแกรม ข้อมูลทุกอย่างอยู่ในเครื่องคุณ",
    fact3_title: "รหัสผ่านอยู่ใน Windows",
    fact3_desc: "เก็บใน Windows Credential Manager ไม่อยู่ในฐานข้อมูลของโปรแกรม ไฟล์ log หรือไฟล์ที่ export ออกไป",
    fact4_title: "ตรวจ host key ทุกครั้ง",
    fact4_desc: "เซิร์ฟเวอร์ใหม่ต้องยืนยัน fingerprint SHA-256 ก่อน ถ้า key เปลี่ยน โปรแกรมจะไม่เชื่อมต่อจนกว่าคุณตัดสินใจ",

    // Features
    feat_kicker: "ความสามารถ",
    feat_heading: "งานที่ผู้ดูแลระบบทำทุกวัน อยู่ในหน้าต่างเดียว",
    feat1_title: "เปิดหลายเทอร์มินัลในแท็บเดียว",
    feat1_desc: "แบ่งหน้าจอได้ 2 หรือ 4 ช่อง แล้วลากเส้นแบ่งเพื่อปรับขนาด ใช้ PowerShell, Command Prompt, Git Bash, WSL และ SSH ร่วมกันได้ในแท็บเดียว โปรแกรมตรวจหา shell ที่ติดตั้งไว้ในเครื่องให้เอง",
    feat1_t1: "รองรับสี 256 สี, truecolor และ Unicode",
    feat1_t2: "ค้นหาข้อความในเทอร์มินัล, export ผลลัพธ์ และบันทึก log ลงไฟล์",
    feat1_t3: "ปรับฟอนต์, เคอร์เซอร์ และชุดสีของเทอร์มินัลได้",

    feat2_title: "จัดการไฟล์บนเซิร์ฟเวอร์ด้วย SFTP",
    feat2_desc: "เปิด SFTP จาก SSH ที่เชื่อมต่ออยู่ได้ทันทีโดยไม่ต้องล็อกอินซ้ำ ลากไฟล์จาก File Explorer มาวางเพื่ออัปโหลด ดูความคืบหน้าและยกเลิกได้ ไฟล์ที่ส่งไม่ครบจะถูกลบทิ้ง ไม่ค้างอยู่บนเซิร์ฟเวอร์",
    feat2_t1: "สร้าง, เปลี่ยนชื่อ, ลบไฟล์และโฟลเดอร์ โดยถามยืนยันก่อนลบหรือเขียนทับ",
    feat2_t2: "แก้ไฟล์ข้อความบนเซิร์ฟเวอร์ในโปรแกรม: บันทึกลงไฟล์ชั่วคราวก่อนแล้วจึงแทนที่ไฟล์เดิม ถ้าบันทึกไม่สำเร็จ ไฟล์เดิมยังอยู่ครบ",
    feat2_t3: "ส่งไฟล์ใหญ่แบบทยอยอ่าน ไม่โหลดทั้งไฟล์เข้าหน่วยความจำ",

    feat3_title: "สั่งงานหลายเซิร์ฟเวอร์พร้อมกัน",
    feat3_desc: "เลือกเทอร์มินัลที่จะรับคำสั่ง แล้วพิมพ์ครั้งเดียว ก่อนส่งจะมีหน้าต่างยืนยันที่แสดงรายชื่อเครื่องปลายทางทั้งหมด ช่องที่ถูกเลือกมีกรอบสีเหลืองให้เห็นชัดตลอดเวลา",
    feat3_t1: 'คำสั่งที่อาจทำความเสียหาย เช่น <code>rm -rf</code>, <code>mkfs</code>, <code>reboot</code> หรือ <code>DROP TABLE</code> จะมีคำเตือนสีแดง',
    feat3_t2: "โหมดพิมพ์พร้อมกัน: ทุกปุ่มที่กดในช่องหนึ่งถูกส่งไปยังช่องอื่นที่เลือกไว้ด้วย",

    card_hostkey_title: "ยืนยันตัวตนเซิร์ฟเวอร์",
    card_hostkey_desc: 'แสดง fingerprint SHA-256 ก่อนเชื่อมต่อเซิร์ฟเวอร์ใหม่ทุกครั้ง ไม่มีตัวเลือก "เชื่อทุกเครื่อง" ให้กดข้าม',
    card_tunnels_title: "SSH tunnel",
    card_tunnels_desc: "ส่งต่อพอร์ตแบบ local (-L), remote (-R) และ SOCKS5 (-D) เริ่ม หยุด หรือเริ่มใหม่ได้ แจ้งเตือนเมื่อพอร์ตถูกใช้อยู่แล้ว",
    card_keys_title: "จัดการ SSH key",
    card_keys_desc: "สร้าง key แบบ Ed25519 หรือ RSA พร้อมรหัสผ่าน นำเข้า key เดิม คัดลอกหรือ export public key และเปลี่ยนรหัสผ่านของ key ได้",
    card_snippets_title: "ชุดคำสั่งที่ใช้บ่อย",
    card_snippets_desc: 'บันทึกคำสั่งแยกหมวด ใส่ตัวแปรแบบ <code>{{service}}</code> ให้โปรแกรมถามค่าตอนใช้ และแสดงคำสั่งเต็มให้ดูก่อนส่งทุกครั้ง',
    card_net_title: "เครื่องมือเครือข่าย",
    card_net_desc: "Ping, Traceroute, DNS, Reverse DNS, ทดสอบพอร์ต TCP และดูข้อมูลการ์ดเครือข่าย ทุกอย่างทำงานเมื่อคุณกดเท่านั้น และยกเลิกกลางคันได้",
    card_other_title: "รายละเอียดอื่นๆ",
    card_other_t1: "ล็อกอินด้วยรหัสผ่าน, private key, SSH agent ของ Windows หรือ keyboard-interactive",
    card_other_t2: "เชื่อมต่อผ่านเครื่องกลาง (ProxyJump)",
    card_other_t3: 'จัด session เป็นโฟลเดอร์ ใส่แท็ก ค้นหา และนำเข้าจาก <code>~/.ssh/config</code>',
    card_other_t4: "สำรองและกู้คืนข้อมูลได้จากหน้าตั้งค่า",
    card_other_t5: "ธีมมืดและธีมสว่าง",

    // Guide
    guide_kicker: "วิธีใช้งาน",
    guide_heading: "เริ่มต้นใช้งานใน 6 ขั้นตอน",
    guide_step1_title: "ติดตั้งหรือแตกไฟล์",
    guide_step1_desc: 'ดับเบิลคลิก <code>NexaTerm-Setup-x64.exe</code> แล้วทำตามตัวช่วยติดตั้ง หรือแตกไฟล์ <code>NexaTerm-Portable-x64.zip</code> แล้วเปิด <code>NexaTerm.exe</code> ได้เลย',
    guide_step2_title: "เปิดเทอร์มินัลบนเครื่อง",
    guide_step2_desc: 'กด <kbd>Ctrl</kbd> + <kbd>T</kbd> เพื่อเปิด shell เริ่มต้น หรือกดลูกศรข้างปุ่มเทอร์มินัลบนแถบเครื่องมือเพื่อเลือก PowerShell, Command Prompt, Git Bash หรือ WSL',
    guide_step3_title: "เพิ่ม session SSH",
    guide_step3_desc: 'กด <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>N</kbd> ใส่ชื่อเครื่อง พอร์ต ชื่อผู้ใช้ และวิธีล็อกอิน ถ้าใส่รหัสผ่านไว้ รหัสจะถูกเก็บใน Windows Credential Manager หรือเว้นว่างไว้ให้โปรแกรมถามตอนเชื่อมต่อก็ได้',
    guide_step4_title: "เชื่อมต่อครั้งแรก",
    guide_step4_desc: 'ดับเบิลคลิก session ในแถบด้านซ้าย โปรแกรมจะแสดง fingerprint ของเซิร์ฟเวอร์ ให้เทียบกับค่าที่ผู้ดูแลเซิร์ฟเวอร์ให้มา ถ้าตรงกันจึงกด "Trust and connect"',
    guide_step5_title: "เปิด SFTP",
    guide_step5_desc: 'ขณะอยู่ในเทอร์มินัล SSH ให้กดปุ่ม SFTP บนแถบเครื่องมือ หรือคลิกขวาที่ session แล้วเลือก "Open SFTP browser" ลากไฟล์มาวางเพื่ออัปโหลด ดับเบิลคลิกไฟล์ข้อความเพื่อแก้ไข',
    guide_step6_title: "แบ่งหน้าจอและสั่งงานหลายเครื่อง",
    guide_step6_desc: 'คลิกขวาที่ session แล้วเลือก "Connect in split pane" เพื่อเปิดในช่องใหม่ จากนั้นกดปุ่ม multi-execution บนแถบเครื่องมือ เลือกเครื่องปลายทาง พิมพ์คำสั่ง แล้วกด Broadcast',
    guide_more: 'คู่มือฉบับเต็มอยู่ที่ <a href="https://github.com/bom-sudo/NexaTerm/blob/main/docs/USER_GUIDE.md" target="_blank" rel="noopener">docs/USER_GUIDE.md</a>',

    // Shortcuts
    sc_kicker: "คีย์ลัด",
    sc_heading: "คีย์ลัดที่ใช้บ่อย",
    sc_th_key: "คีย์",
    sc_th_action: "การทำงาน",
    sc_new_session: "สร้าง session ใหม่",
    sc_new_term: "เปิดเทอร์มินัลใหม่",
    sc_close_tab: "ปิดแท็บปัจจุบัน",
    sc_nav_tab: "ไปแท็บถัดไป / ก่อนหน้า",
    sc_search_term: "ค้นหาข้อความในเทอร์มินัล",
    sc_search_session: "ค้นหา session",
    sc_copy_paste: "คัดลอก / วาง",
    sc_font_size: "ขยาย / ย่อ / คืนขนาดตัวอักษร",
    sc_note: 'ขณะพิมพ์ในเทอร์มินัล <kbd>Ctrl</kbd> + <kbd>W</kbd> และ <kbd>Ctrl</kbd> + <kbd>T</kbd> จะถูกส่งให้ shell (bash ใช้ <kbd>Ctrl</kbd> + <kbd>W</kbd> ลบคำ) ให้ใช้ <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>W</kbd> และ <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>T</kbd> แทน หรือเปลี่ยนได้ในหน้าตั้งค่า',

    // Privacy & Security
    priv_kicker: "ความปลอดภัย",
    priv_heading: "ข้อมูลของคุณเก็บไว้ที่ไหน",
    priv_th_data: "ข้อมูล",
    priv_th_location: "ที่เก็บ",
    priv_row1_data: "Session, โฟลเดอร์, การตั้งค่า, known hosts",
    priv_row1_loc: 'ฐานข้อมูล SQLite ใน <code>%APPDATA%\\NexaTerm</code> (รุ่นพกพาเก็บใน <code>portable-data\\</code>)',
    priv_row2_data: "รหัสผ่านและรหัสของ key",
    priv_row2_loc: "Windows Credential Manager ของผู้ใช้ Windows คนนั้น",
    priv_row3_data: "Private key",
    priv_row3_loc: "อยู่ในไฟล์ที่คุณเลือกตามเดิม โปรแกรมจำไว้แค่ตำแหน่งไฟล์",
    priv_row4_data: "Log ของโปรแกรม",
    priv_row4_loc: 'โฟลเดอร์ <code>logs\\</code> ข้อความที่เป็นรหัสผ่านจะถูกปิดบังก่อนเขียน',
    priv_note: 'ไฟล์ session ที่ export ออกไปและไฟล์สำรองฐานข้อมูลไม่มีรหัสผ่านอยู่ข้างใน รายละเอียดเพิ่มเติมอยู่ใน <a href="https://github.com/bom-sudo/NexaTerm/blob/main/SECURITY.md" target="_blank" rel="noopener">SECURITY.md</a>',

    // Downloads
    dl_kicker: "รุ่นล่าสุด",
    dl_heading: "ดาวน์โหลด NexaTerm 1.0.0",
    dl_sub: "สำหรับ Windows 10 (1809 ขึ้นไป) และ Windows 11 แบบ 64 บิต ไม่ต้องใช้สิทธิ์ผู้ดูแลระบบ",
    dl_badge_rec: "แนะนำ",
    dl_badge_small: "ไฟล์เล็ก",
    dl_badge_port: "ไม่ต้องติดตั้ง",
    dl_offline_title: "ตัวติดตั้งแบบออฟไลน์",
    dl_offline_desc: "มีตัวติดตั้ง WebView2 Runtime อยู่ในไฟล์ ติดตั้งได้โดยไม่ต้องต่ออินเทอร์เน็ต ระหว่างติดตั้งต้องมีพื้นที่ว่างในไดรฟ์ C: ประมาณ 300 MB",
    dl_offline_btn: "ดาวน์โหลดตัวติดตั้ง",
    dl_web_title: "ตัวติดตั้งแบบออนไลน์",
    dl_web_desc: "ตัวติดตั้งเดียวกัน แต่จะดาวน์โหลด WebView2 จาก Microsoft เฉพาะเมื่อเครื่องยังไม่มี Windows 11 และ Windows 10 ที่อัปเดตแล้วมีอยู่แล้ว",
    dl_web_btn: "ดาวน์โหลด",
    dl_port_title: "รุ่นพกพา",
    dl_port_desc: "แตกไฟล์ไว้ที่ไหนก็ได้ รวมถึงแฟลชไดรฟ์ ข้อมูลทั้งหมดอยู่ในโฟลเดอร์ portable-data\\ ข้างโปรแกรม ต้องมี WebView2 ในเครื่องอยู่แล้ว",
    dl_port_btn: "ดาวน์โหลด",
    verify_title: "ตรวจสอบไฟล์ก่อนติดตั้ง",
    verify_desc: 'ไฟล์ในรุ่นนี้ยังไม่ได้ลงลายเซ็นดิจิทัล Windows SmartScreen อาจขึ้นคำเตือนตอนเปิดครั้งแรก ให้เทียบค่า SHA-256 กับไฟล์ <code>SHA256SUMS.txt</code> ในหน้า Release ก่อน แล้วจึงกด "More info" และ "Run anyway"',
    copy_btn: "คัดลอก",
    copied_btn: "คัดลอกแล้ว",
    dl_more: 'ไฟล์ทุกรุ่นและบันทึกการเปลี่ยนแปลงอยู่ที่ <a href="https://github.com/bom-sudo/NexaTerm/releases" target="_blank" rel="noopener">หน้า Releases บน GitHub</a>',

    // FAQ
    faq_heading: "คำถามที่พบบ่อย",
    faq_q1: "ใช้ฟรีจริงไหม มีเงื่อนไขอะไรบ้าง",
    faq_a1: "ฟรีทุกฟังก์ชัน เผยแพร่ภายใต้สัญญาอนุญาต MIT ใช้งานส่วนตัวหรือในองค์กรได้ แก้ไขและนำไปแจกจ่ายต่อได้ ไม่มีรุ่นเสียเงินและไม่มีระยะทดลองใช้",
    faq_q2: "โปรแกรมส่งข้อมูลอะไรออกไปบ้าง",
    faq_a2: 'ไม่ส่งข้อมูลการใช้งานใดๆ ไม่มี telemetry และไม่ตรวจหาอัปเดตเอง การเชื่อมต่อออกไปข้างนอกเกิดขึ้นเฉพาะเมื่อคุณเชื่อมต่อเซิร์ฟเวอร์ หรือกดปุ่ม "Public IP" ในหน้าเครื่องมือเครือข่าย',
    faq_q3: "ต้องใช้สิทธิ์ผู้ดูแลระบบไหม",
    faq_a3: 'ไม่ต้อง ตัวติดตั้งติดตั้งให้เฉพาะผู้ใช้ปัจจุบัน (ค่าเริ่มต้นคือ <code>%LOCALAPPDATA%\\NexaTerm</code>) ส่วนรุ่นพกพาไม่ต้องติดตั้งเลย',
    faq_q4: "WebView2 คืออะไร ต้องติดตั้งเพิ่มไหม",
    faq_a4: "WebView2 เป็นส่วนประกอบของ Microsoft ที่ NexaTerm ใช้แสดงหน้าจอ Windows 11 และ Windows 10 ที่อัปเดตแล้วมีอยู่แล้ว ถ้าเครื่องยังไม่มี ตัวติดตั้งจะติดตั้งให้ ส่วนรุ่นพกพาต้องติดตั้ง WebView2 เองก่อน",
    faq_q5: "ย้ายรุ่นพกพาไปเครื่องอื่น รหัสผ่านไปด้วยไหม",
    faq_a5: 'ไม่ไป เพราะรหัสผ่านเก็บใน Windows Credential Manager ของผู้ใช้แต่ละคน Session, โฟลเดอร์, การตั้งค่าและ known hosts ย้ายไปพร้อมโฟลเดอร์ <code>portable-data\\</code> แต่โปรแกรมจะถามรหัสผ่านใหม่บนเครื่องใหม่',
    faq_q6: "รองรับ RDP, VNC, X11, Telnet หรือ Serial ไหม",
    faq_a6: 'ยังไม่รองรับในรุ่น 1.0.0 และโปรแกรมจะไม่แสดงตัวเลือกเหล่านี้จนกว่าจะใช้งานได้จริง แผนงานอยู่ใน <a href="https://github.com/bom-sudo/NexaTerm/blob/main/DEVELOPMENT_STATUS.md" target="_blank" rel="noopener">DEVELOPMENT_STATUS.md</a>',
    faq_q7: "เกี่ยวข้องกับโปรแกรมเชิงพาณิชย์ตัวอื่นไหม",
    faq_a7: "ไม่เกี่ยวข้อง NexaTerm เป็นโปรเจกต์อิสระที่เขียนขึ้นใหม่ทั้งหมดด้วย Rust และ TypeScript ไม่มีโค้ดหรือไฟล์ภาพจากโปรแกรมอื่น",

    // Tech
    tech_kicker: "สำหรับนักพัฒนา",
    tech_heading: "สร้างด้วยอะไร",
    tech_t1_title: "Tauri 2 และ Rust",
    tech_t1_desc: "งานที่ต้องใช้สิทธิ์ของระบบ เช่น เปิด process, เชื่อมต่อเครือข่าย และอ่านเขียนไฟล์ ทำในฝั่ง Rust ทั้งหมด หน้าจอเรียกใช้ผ่านคำสั่งที่ตรวจสอบข้อมูลแล้วเท่านั้น",
    tech_t2_title: "russh และ russh-sftp",
    tech_t2_desc: "ไลบรารี SSH และ SFTP ที่เขียนด้วย Rust ทั้งหมด ทำงานแบบ async บน Tokio ไม่ต้องพึ่ง OpenSSL",
    tech_t3_title: "ConPTY และ xterm.js",
    tech_t3_desc: "เทอร์มินัลบนเครื่องใช้ Windows ConPTY ผ่าน portable-pty ส่วนการแสดงผลใช้ xterm.js",
    tech_t4_title: "SQLite",
    tech_t4_desc: "ข้อมูลทั้งหมดอยู่ในไฟล์ SQLite ไฟล์เดียว มีระบบ migration สำหรับอัปเกรดรุ่น",
    tech_more: 'ต้องใช้ Node.js 22 ขึ้นไป, Rust และ C compiler รายละเอียดอยู่ใน <a href="https://github.com/bom-sudo/NexaTerm/blob/main/docs/DEVELOPER.md" target="_blank" rel="noopener">docs/DEVELOPER.md</a>',

    // Footer
    footer_tag: "โปรแกรมฟรีและโอเพนซอร์สภายใต้สัญญาอนุญาต MIT",
    footer_gh: "ซอร์สโค้ดบน GitHub",
    footer_releases: "Releases",
    footer_issues: "แจ้งปัญหา",
    footer_license: "สัญญาอนุญาต MIT",
    footer_copy: "© 2026 NexaTerm Project. โปรเจกต์อิสระ ไม่มีความเกี่ยวข้องกับผู้พัฒนาโปรแกรมเชิงพาณิชย์รายใด"
  },

  en: {
    page_title: "NexaTerm - Free Open-Source SSH & SFTP Terminal for Windows",
    page_desc: "NexaTerm combines a tabbed SSH terminal, SFTP file manager, SSH tunnels, multi-execution, and network tools into one Windows 10/11 workstation. Free & open source (MIT). No account required, zero telemetry.",
    
    // Navigation
    nav_brand: "NexaTerm",
    nav_features: "Features",
    nav_guide: "User Guide",
    nav_privacy: "Security",
    nav_download: "Download",
    nav_faq: "FAQ",
    nav_download_btn: "Download",

    // Hero
    hero_eyebrow: 'NexaTerm 1.0.0 <span>·</span> Free &amp; Open Source (MIT) <span>·</span> Windows 10 &amp; 11',
    hero_title: 'One Workspace.<br><span class="grad">Every Remote Connection.</span>',
    hero_lead: "A tabbed terminal, SSH client, SFTP file manager, SSH tunnels, and network tools for system administrators, in one Windows desktop app. No account required, no paid tier, and zero telemetry.",
    hero_cta_download: "Download for Windows",
    hero_cta_guide: "View User Guide",
    hero_note: "Per-user installer, no admin rights needed. Or use the portable edition with zero installation.",
    hero_shot_caption: "Local PowerShell and remote SSH side-by-side in one tab. All screenshots show the real application connected to the local mock sshd test server, which is why hosts read 127.0.0.1.",

    // Facts
    fact1_title: "100% Free All Features",
    fact1_desc: "MIT licensed. No paid tiers, no license keys, and no limits on sessions or tunnels.",
    fact2_title: "Zero Telemetry",
    fact2_desc: "No analytics, no tracking, and no phoning home on startup. Everything stays local on your machine.",
    fact3_title: "Secrets in Windows",
    fact3_desc: "Stored in Windows Credential Manager (DPAPI), never in the SQLite database, logs, or exports.",
    fact4_title: "Strict Host-Key Check",
    fact4_desc: "New servers require SHA-256 fingerprint verification. If a key changes, the connection is blocked until you decide.",

    // Features
    feat_kicker: "Features",
    feat_heading: "Everyday Sysadmin Workflow, in One Window",
    feat1_title: "Split Terminals in a Single Tab",
    feat1_desc: "Split views into 2 panes or a 4-pane grid with draggable dividers. Use PowerShell, CMD, Git Bash, WSL, and SSH side-by-side in one tab. Auto-detects all installed local shells.",
    feat1_t1: "Supports 256-color, truecolor, and Unicode 11",
    feat1_t2: "Search terminal buffer, export output, and file session logging",
    feat1_t3: "Customizable fonts, cursors, and 4 color themes",

    feat2_title: "Manage Remote Files via SFTP",
    feat2_desc: "Open SFTP from an active SSH session instantly without re-authenticating. Drag and drop files from Windows Explorer to upload. View progress and cancel queue. Partial transfers are safely cleaned up.",
    feat2_t1: "Create, rename, delete files & folders with confirmation prompts",
    feat2_t2: "In-app remote text editor with atomic safe save: saves to a temp file first so original files are never corrupted",
    feat2_t3: "Streaming large file transfers without loading entire files into RAM",

    feat3_title: "Multi-Execution Across Servers",
    feat3_desc: "Select target terminals and type once. A confirmation dialog reviews all target hosts before execution. Selected panes are highlighted with a prominent amber border.",
    feat3_t1: 'Destructive commands such as <code>rm -rf</code>, <code>mkfs</code>, <code>reboot</code> or <code>DROP TABLE</code> trigger red warnings',
    feat3_t2: "Keystroke mirroring: every key typed in one pane is mirrored live to selected panes",

    card_hostkey_title: "Host Key Verification",
    card_hostkey_desc: 'Displays the SHA-256 fingerprint before connecting to any new server. No dangerous "Trust all" bypass button.',
    card_tunnels_title: "SSH Tunnels",
    card_tunnels_desc: "Local (-L), remote (-R), and dynamic SOCKS5 (-D) forwarding with live port-conflict detection and status.",
    card_keys_title: "SSH Key Manager",
    card_keys_desc: "Generate Ed25519 or RSA keys with passphrases, import existing keys, copy public keys, and change passphrases.",
    card_snippets_title: "Command Snippets",
    card_snippets_desc: 'Save categorized snippets with <code>{{service}}</code> variables that prompt at runtime, previewed before execution.',
    card_net_title: "Network Tools",
    card_net_desc: "Ping, traceroute, DNS, reverse DNS, TCP port testing, and local interfaces. Runs on request only and fully cancellable.",
    card_other_title: "Other Highlights",
    card_other_t1: "Auth via password, private key, Windows OpenSSH agent, or keyboard-interactive",
    card_other_t2: "Connect via jump hosts (ProxyJump)",
    card_other_t3: 'Organize sessions into folders, tag, search, and import from <code>~/.ssh/config</code>',
    card_other_t4: "Built-in backup and restore from settings",
    card_other_t5: "Dark and light themes",

    // Guide
    guide_kicker: "User Guide",
    guide_heading: "Get Started in 6 Simple Steps",
    guide_step1_title: "Install or Extract",
    guide_step1_desc: 'Run <code>NexaTerm-Setup-x64.exe</code> wizard or extract <code>NexaTerm-Portable-x64.zip</code> anywhere and launch <code>NexaTerm.exe</code>.',
    guide_step2_title: "Open Local Terminal",
    guide_step2_desc: 'Press <kbd>Ctrl</kbd> + <kbd>T</kbd> for the default shell, or click the toolbar arrow to select PowerShell, CMD, Git Bash, or WSL.',
    guide_step3_title: "Add an SSH Session",
    guide_step3_desc: 'Press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>N</kbd>, enter host, port, username, and auth. Passwords are saved in Windows Credential Manager or prompted on connect.',
    guide_step4_title: "First Connection",
    guide_step4_desc: 'Double-click the session. Compare the displayed SHA-256 fingerprint with your server, and click "Trust and connect".',
    guide_step5_title: "Open SFTP File Manager",
    guide_step5_desc: 'Inside an SSH terminal, click the toolbar SFTP button or right-click the session and choose "Open SFTP browser". Drag files to upload, double-click to edit.',
    guide_step6_title: "Split Panes & Broadcast",
    guide_step6_desc: 'Right-click a session and select "Connect in split pane". Click multi-execution on the toolbar, pick target panes, type a command, and click Broadcast.',
    guide_more: 'Full documentation available at <a href="https://github.com/bom-sudo/NexaTerm/blob/main/docs/USER_GUIDE.md" target="_blank" rel="noopener">docs/USER_GUIDE.md</a>',

    // Shortcuts
    sc_kicker: "Shortcuts",
    sc_heading: "Essential Keyboard Shortcuts",
    sc_th_key: "Key",
    sc_th_action: "Action",
    sc_new_session: "New session dialog",
    sc_new_term: "New terminal tab",
    sc_close_tab: "Close active tab",
    sc_nav_tab: "Next / previous tab",
    sc_search_term: "Search in terminal buffer",
    sc_search_session: "Search saved sessions",
    sc_copy_paste: "Copy / paste clipboard",
    sc_font_size: "Zoom in / out / reset font size",
    sc_note: 'While typing in a terminal, <kbd>Ctrl</kbd> + <kbd>W</kbd> and <kbd>Ctrl</kbd> + <kbd>T</kbd> go to the shell by default (bash uses <kbd>Ctrl</kbd> + <kbd>W</kbd> to delete a word). Use <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>W</kbd> and <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>T</kbd> instead, or configure in Settings.',

    // Privacy & Security
    priv_kicker: "Security",
    priv_heading: "Where Your Data Lives",
    priv_th_data: "Data",
    priv_th_location: "Storage Location",
    priv_row1_data: "Sessions, folders, settings, known hosts",
    priv_row1_loc: 'SQLite database in <code>%APPDATA%\\NexaTerm</code> (portable edition uses <code>portable-data\\</code>)',
    priv_row2_data: "Passwords and key passphrases",
    priv_row2_loc: "Windows Credential Manager (DPAPI) of the logged-in user",
    priv_row3_data: "Private keys",
    priv_row3_loc: "Stored in original user files; NexaTerm only saves the path",
    priv_row4_data: "Application logs",
    priv_row4_loc: 'Folder <code>logs\\</code>; secrets and passwords are redacted before writing',
    priv_note: 'Exported session files and database backups contain zero passwords. Details in <a href="https://github.com/bom-sudo/NexaTerm/blob/main/SECURITY.md" target="_blank" rel="noopener">SECURITY.md</a>',

    // Downloads
    dl_kicker: "Latest Release",
    dl_heading: "Download NexaTerm 1.0.0",
    dl_sub: "For Windows 10 (1809+) and Windows 11 x64. No administrator privileges required.",
    dl_badge_rec: "Recommended",
    dl_badge_small: "Lightweight",
    dl_badge_port: "Zero Install",
    dl_offline_title: "Offline Installer",
    dl_offline_desc: "Bundles the WebView2 Runtime inside the installer. Works completely offline. Requires ~300 MB free space in C: during install.",
    dl_offline_btn: "Download Installer",
    dl_web_title: "Web Installer",
    dl_web_desc: "Same installer, but downloads WebView2 from Microsoft only if missing on older Windows 10 machines.",
    dl_web_btn: "Download",
    dl_port_title: "Portable Edition",
    dl_port_desc: "Extract anywhere, including USB thumb drives. All data is kept in the portable-data\\ folder next to the executable.",
    dl_port_btn: "Download",
    verify_title: "Verify File Integrity",
    verify_desc: 'Releases are not yet code-signed; Windows SmartScreen may show an alert. Compare the SHA-256 hash with <code>SHA256SUMS.txt</code> on the Release page first, then click "More info" and "Run anyway".',
    copy_btn: "Copy",
    copied_btn: "Copied!",
    dl_more: 'All releases and changelogs are on the <a href="https://github.com/bom-sudo/NexaTerm/releases" target="_blank" rel="noopener">GitHub Releases page</a>',

    // FAQ
    faq_heading: "Frequently Asked Questions",
    faq_q1: "Is NexaTerm really 100% free?",
    faq_a1: "Yes, every feature is free and open source under the MIT License. Use it personally or in organizations, modify it, and distribute it. No paid tiers, no trial periods.",
    faq_q2: "What data does NexaTerm send out?",
    faq_a2: 'Zero data. No telemetry, no usage tracking, and no automated update checks. Network connections only happen when you connect to a server or click "Public IP" in Network Tools.',
    faq_q3: "Do I need administrator rights?",
    faq_a3: 'No. The setup installer installs per-user (defaulting to <code>%LOCALAPPDATA%\\NexaTerm</code>), and the portable edition requires no installation at all.',
    faq_q4: "What is WebView2? Do I need to install it?",
    faq_a4: "WebView2 is Microsoft's embedded browser component used for the UI. Windows 11 and up-to-date Windows 10 include it out of the box. The offline installer bundles it.",
    faq_q5: "If I move the portable edition to another PC, do passwords follow?",
    faq_a5: 'No, because passwords are saved in the Windows Credential Manager of each individual user. Sessions, folders, and settings migrate with <code>portable-data\\</code>, but you will be prompted for passwords on the new PC.',
    faq_q6: "Does it support RDP, VNC, X11, Telnet, or Serial?",
    faq_a6: 'Not yet in v1.0.0. No UI is shown for these until they are fully functional. Roadmap is in <a href="https://github.com/bom-sudo/NexaTerm/blob/main/DEVELOPMENT_STATUS.md" target="_blank" rel="noopener">DEVELOPMENT_STATUS.md</a>',
    faq_q7: "Is it affiliated with commercial tools?",
    faq_a7: "No. NexaTerm is an independent clean-room project written from scratch with Rust and TypeScript. It contains zero code or assets from other commercial software.",

    // Tech
    tech_kicker: "For Developers",
    tech_heading: "Built With Modern Technologies",
    tech_t1_title: "Tauri 2 & Rust",
    tech_t1_desc: "Privileged operations (processes, network sockets, file I/O) are owned exclusively by Rust. UI calls validated IPC commands only.",
    tech_t2_title: "russh & russh-sftp",
    tech_t2_desc: "Pure Rust async SSH and SFTP library running on Tokio. Fast, secure, and free of OpenSSL C-bindings.",
    tech_t3_title: "ConPTY & xterm.js",
    tech_t3_desc: "Local terminals utilize Windows ConPTY via portable-pty, rendered at high performance with xterm.js.",
    tech_t4_title: "SQLite",
    tech_t4_desc: "All session and configuration data resides in a single local SQLite file with migration versioning.",
    tech_more: 'Prerequisites: Node.js 22+, Rust, and C toolchain. Documentation in <a href="https://github.com/bom-sudo/NexaTerm/blob/main/docs/DEVELOPER.md" target="_blank" rel="noopener">docs/DEVELOPER.md</a>',

    // Footer
    footer_tag: "Free and open-source software under the MIT License",
    footer_gh: "Source code on GitHub",
    footer_releases: "Releases",
    footer_issues: "Report Issue",
    footer_license: "MIT License",
    footer_copy: "© 2026 NexaTerm Project. An independent clean-room project with no affiliation to commercial vendors."
  }
};

/**
 * Switch language function
 * @param {'th'|'en'} lang
 */
function setLanguage(lang) {
  if (!translations[lang]) return;

  const t = translations[lang];

  // Update HTML lang attribute
  document.documentElement.lang = lang;

  // Update title & description
  if (t.page_title) document.title = t.page_title;
  const metaDesc = document.querySelector('meta[name="description"]');
  if (metaDesc && t.page_desc) metaDesc.setAttribute('content', t.page_desc);

  // Update elements with data-i18n
  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const key = el.getAttribute('data-i18n');
    if (t[key] !== undefined) {
      el.textContent = t[key];
    }
  });

  // Update elements with data-i18n-html
  document.querySelectorAll('[data-i18n-html]').forEach((el) => {
    const key = el.getAttribute('data-i18n-html');
    if (t[key] !== undefined) {
      el.innerHTML = t[key];
    }
  });

  // Update active state on language buttons
  document.querySelectorAll('.lang-btn').forEach((btn) => {
    const btnLang = btn.getAttribute('data-lang');
    const isActive = btnLang === lang;
    btn.classList.toggle('active', isActive);
    btn.setAttribute('aria-pressed', isActive ? 'true' : 'false');
  });

  // Save to localStorage
  try {
    localStorage.setItem('nexaterm_lang', lang);
  } catch (e) {
    // ignore
  }
}

// Initialize on DOM ready
document.addEventListener('DOMContentLoaded', () => {
  // Determine preferred language: localStorage -> browser language -> default 'th'
  let savedLang = null;
  try {
    savedLang = localStorage.getItem('nexaterm_lang');
  } catch (e) {}

  if (!savedLang) {
    const browserLang = (navigator.language || navigator.userLanguage || '').toLowerCase();
    savedLang = browserLang.startsWith('th') ? 'th' : 'en';
  }

  // Bind clicks to language switch buttons
  document.querySelectorAll('.lang-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      const targetLang = btn.getAttribute('data-lang');
      setLanguage(targetLang);
    });
  });

  // Apply language
  setLanguage(savedLang);
});

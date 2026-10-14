#!/usr/bin/env python3
"""Capture the native POS Heart debug build with isolated, fictional data."""
import argparse
import html
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).resolve().parent
CASES = []

def add(group, slug, label, page, preset='page', fixture='main', login=True):
    CASES.append(dict(group=group, slug=slug, label=label, page=page, preset=preset, fixture=fixture, login=login))

for slug, label, page in [
    ('home','หน้าหลัก','home'),('sell','ขาย','sell'),('products','สินค้า','products'),
    ('history','ประวัติขาย','history'),('shift','กะและลิ้นชัก','shift'),
    ('reports','รายงานประจำวัน','reports'),('settings','ตั้งค่า','settings'),('activity','ประวัติกิจกรรม','activity')]:
    add('01-main',slug,label,page,'activity' if page=='activity' else 'page')
add('01-main','setup','เริ่มต้นร้าน','home',None,'fresh',False)
add('01-main','lock','หน้าลงชื่อเข้าใช้ / ล็อก','home',None,'main',False)
add('01-main','open-shift','เปิดกะ (ยังไม่มีกะ)','shift',None,'empty',False)

for slug,label in [
    ('sell-held','บิลพัก'),('sell-note','หมายเหตุรายการ'),('sell-options','เลือกตัวเลือกสินค้า'),
    ('sell-discount','ส่วนลดรายการ'),('sell-review','ยืนยันรับโอน / QR'),
    ('sell-split','แบ่งวิธีชำระ'),('sell-split-review','ยืนยันแบ่งชำระ'),('bill-split','แยกบิล'),
    ('sell-coupon','ใส่คูปอง'),('sell-large','ยืนยันเงินสดจำนวนมาก'),('paid','ชำระเสร็จ'),
    ('table-bill','บิลโต๊ะ'),('table-pay','ชำระบิลโต๊ะ'),('table-move','ย้ายโต๊ะ'),('kitchen','จอครัว')]:
    add('02-sell',slug,label,'sell',slug,'sets' if slug=='sell-options' else 'main')
add('02-sell','tables','ผังโต๊ะ','tables')

for slug,label in [
    ('product','แก้ไขสินค้า'),('product-new','เพิ่มสินค้า'),('product-cost','ต้นทุนสินค้า'),
    ('product-recipe','สูตรและสูตรตามตัวเลือก'),('bulk','เลือกหลายสินค้า'),
    ('stock-count','นับสต็อก'),('stock-remove','นำสต็อกออก'),
    ('csv-import','นำเข้าสินค้า CSV'),('csv-import-errors','ข้อผิดพลาดการนำเข้า CSV'),
    ('categories','จัดการหมวดหมู่'),('category-rename','เปลี่ยนชื่อ / รวมหมวด'),
    ('option-sets','ชุดตัวเลือกร่วม'),('option-set-editor','แก้ไขชุดตัวเลือกร่วม'),
    ('option-set-bulk','ผูกชุดตัวเลือกหลายสินค้า'),('ingredients','วัตถุดิบ'),('ingredient-history','ประวัติวัตถุดิบ')]:
    add('03-catalog',slug,label,'products',slug,'sets' if slug.startswith('option-set') else 'main')

for slug,label in [
    ('purchasing','รายการใบสั่งซื้อ'),('suppliers','ผู้ขาย'),('supplier-editor','แก้ไขผู้ขาย'),
    ('purchase-create','สร้างใบสั่งซื้อ'),('purchase-create-confirm','ยืนยันใบสั่งซื้อ'),
    ('purchase-detail','รายละเอียดและประวัติรับของ'),('purchase-receive','รับของบางส่วน'),
    ('purchase-receive-confirm','ยืนยันรับของ'),('purchase-cancel','ยกเลิกส่วนที่เหลือ')]:
    add('04-purchasing',slug,label,'products',slug)

for slug,label in [
    ('history-detail','รายละเอียดบิล'),('history-refunded','บิลที่คืนบางรายการ'),('receipt','ใบเสร็จ'),
    ('void','ยกเลิกทั้งบิล'),('refund','คืนบางรายการ'),('invoice','ออกใบกำกับภาษีเต็มรูป'),
    ('history-invoiced','บิลพร้อมใบกำกับและใบลดหนี้'),('credit-note','ตัวอย่างใบลดหนี้'),
    ('receipt-preview','ตัวอย่างพิมพ์ใบเสร็จ'),('check-bill','ตรวจบิลก่อนชำระ')]:
    add('05-history',slug,label,'sell' if slug=='check-bill' else 'history',slug)

for slug,label in [('close','นับและสรุปปิดกะ'),('close-confirm','ยืนยันปิดกะ'),('shift-history','ประวัติกะ')]:
    add('06-shifts',slug,label,'shift',slug)
for slug,label in [('report-week','รายงาน 7 วัน'),('report-month','รายงานรายเดือน'),
    ('report-categories','รายงานหมวดหมู่'),('report-staff','รายงานพนักงาน'),
    ('report-promos','รายงานโปรโมชัน'),('report-ingredients','รายงานวัตถุดิบ'),('report-profit','รายงานกำไรขั้นต้น')]:
    add('07-reports',slug,label,'reports',slug)

for slug,label in [('shop','ร้านและหน้าเริ่มต้น'),('pricing','ราคาและภาษี'),('promotions','โปรโมชัน'),
    ('tables','โต๊ะและโซน'),('printing','เอกสารและการพิมพ์'),('staff','พนักงานและสิทธิ์'),
    ('display','ภาษาและหน้าจอ'),('data','ข้อมูลและการสำรอง'),('vat','ข้อมูล VAT (ด้านล่าง)'),
    ('printer','เครื่องพิมพ์ (ด้านล่าง)'),('roles','รายการบทบาท (ด้านล่าง)')]:
    add('08-settings','settings-'+slug,label,'settings','settings-'+slug)
for slug,label in [('templates-bill','แม่แบบตรวจบิล'),('templates-kitchen','แม่แบบใบครัว'),
    ('staff-editor','แก้ไขพนักงาน'),('role-editor','แก้ไขบทบาทและสิทธิ์'),
    ('promo-editor','แก้ไขโปรโมชัน'),('table-editor','แก้ไขโต๊ะ'),('automatic-backups','สำรองอัตโนมัติ')]:
    add('08-settings',slug,label,'settings',slug)

for slug,label,page in [('palette','ค้นหาและสั่งงาน','home'),('display','เมนูภาษาและธีม','home'),
    ('approval','ขออนุมัติจากผู้จัดการ','history'),('backup-review','ตรวจชุดสำรอง','settings'),
    ('backup-confirm','ยืนยันกู้คืน','settings'),('backup-pending','เตรียมกู้คืนแล้ว','settings')]:
    add('09-dialogs',slug,label,page,slug)


def gallery(records):
    cards=[]
    for r in records:
        if r['status']!='ok': continue
        title=html.escape(r['label']); path=html.escape(r['file'],quote=True)
        search=html.escape((r['label']+' '+r['slug']+' '+r['group']).lower(),quote=True)
        cards.append(f'<article data-theme="{r["theme"]}" data-search="{search}"><a href="{path}" target="_blank"><img src="{path}" loading="lazy" alt="{title}"></a><p>{title} <small>{r["theme"]} · {html.escape(r["slug"])}</small></p></article>')
    content='''<!doctype html><html lang="th"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>POS Heart — Screenshot catalog</title>
<style>body{margin:0;background:#f5f4f0;color:#1f1e1b;font:15px/1.6 system-ui,sans-serif}header{padding:24px;position:sticky;top:0;background:#f5f4f0;border-bottom:1px solid #dedbd3;z-index:1}h1{margin:0;font-size:24px}p{margin:8px 0}button,input{font:inherit;padding:8px 12px;border:1px solid #827d72;border-radius:6px;background:white}button{cursor:pointer}button.active{background:#1f1e1b;color:white}input{margin-left:12px;width:min(300px,50vw)}main{padding:24px;display:grid;grid-template-columns:repeat(auto-fit,minmax(360px,1fr));gap:24px}article{min-width:0}img{display:block;width:100%;height:auto;border:1px solid #dedbd3}small{display:block;color:#625e55}article[hidden]{display:none}@media(max-width:450px){main{grid-template-columns:1fr;padding:12px}}</style>
<header><h1>POS Heart · ทุกหน้าจอ</h1><p>ข้อมูลตัวอย่าง · ไทย · Compact · 1280×820 · คลิกภาพเพื่อเปิดขนาดเต็ม</p><button class="active" data-filter="soft">โค้ง</button> <button data-filter="square">เหลี่ยม</button> <button data-filter="all">ทั้งสองแบบ</button><input type="search" placeholder="ค้นหาหน้าจอ…" aria-label="ค้นหาหน้าจอ"><p id="count"></p></header><main>'''+''.join(cards)+'''</main><script>let theme='soft';const q=document.querySelector('input');function filter(){let count=0;document.querySelectorAll('article').forEach(a=>{a.hidden=!(theme==='all'||a.dataset.theme===theme)||!a.dataset.search.includes(q.value.toLowerCase());if(!a.hidden)count++});document.querySelector('#count').textContent=count+' ภาพ'}document.querySelectorAll('button').forEach(b=>b.onclick=()=>{theme=b.dataset.filter;document.querySelectorAll('button').forEach(x=>x.classList.toggle('active',x===b));filter()});q.oninput=filter;filter()</script></html>'''
    (OUT/'index.html').write_text(content,encoding='utf-8')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--main-fixture',type=Path)
    parser.add_argument('--sets-fixture',type=Path)
    parser.add_argument('--resume',action='store_true',help='Keep completed PNGs and capture missing ones')
    args=parser.parse_args()
    records=[]
    with tempfile.TemporaryDirectory(prefix='pos-heart-capture-') as scratch:
        scratch=Path(scratch)
        fixtures={}
        for name,provided in [('main',args.main_fixture),('sets',args.sets_fixture)]:
            if provided:
                fixtures[name]=provided.resolve()
            else:
                dest=scratch/name
                flags=['--staff','--vat','--restaurant','--tables','--ingredients','--pictures','--promos','--purchases']
                if name=='sets': flags.append('--sets')
                subprocess.run(['cargo','run','-p','pos-desktop','--example','seed_screens','--offline','--',str(dest),*flags],cwd=ROOT,check=True)
                fixtures[name]=dest
        work=scratch/'shot'
        total=len(CASES)*2
        for theme in ['soft','square']:
            for case in CASES:
                group=case['group']; folder=OUT/theme/group; folder.mkdir(parents=True,exist_ok=True)
                path=folder/(case['slug']+'.png')
                record={**case,'theme':theme,'file':str(path.relative_to(OUT)),'status':'ok'}
                if not(args.resume and path.exists()):
                    if work.exists(): shutil.rmtree(work)
                    if case['fixture'] in fixtures: shutil.copytree(fixtures[case['fixture']],work)
                    else:
                        work.mkdir()
                        if case['fixture']=='empty':
                            (work/'shop.json').write_text(json.dumps(dict(version=1,name='ร้านหัวใจ (ตัวอย่าง)',kind='mixed',start_in_sell=False,setup_complete=True)))
                    for name,value in [('shape.txt',theme),('language.txt','th'),('density.txt','compact')]:
                        (work/name).write_text(value)
                    command=[str(ROOT/'target/debug/pos-heart'),'--data-dir',str(work),'--page',case['page'],'--window-size','1280x820','--screenshot',str(path)]
                    if case['preset']:
                        command+=['--preview',('as-owner:' if case['login'] else '')+case['preset']]
                    started=time.monotonic()
                    try:
                        result=subprocess.run(command,cwd=ROOT,capture_output=True,text=True,timeout=35)
                        if result.returncode or not path.exists() or 'Unknown preview:' in result.stderr:
                            record.update(status='failed',error=result.stderr[-1500:])
                    except subprocess.TimeoutExpired:
                        record.update(status='failed',error='Native capture timed out; check that macOS is unlocked.')
                    record['elapsed_seconds']=round(time.monotonic()-started,2)
                records.append(record)
                (OUT/'manifest.json').write_text(json.dumps(dict(size='1280x820',language='th',density='compact',fictional_data=True,captures=records),ensure_ascii=False,indent=2),encoding='utf-8')
                gallery(records)
                print(f'{len(records)}/{total} {theme}/{group}/{case["slug"]}: {record["status"]}',flush=True)
                if record['status']!='ok':
                    print(record.get('error',''),flush=True)
                    raise SystemExit('Capture stopped; rerun with --resume after resolving the failure.')
        print(f'Complete: {total} PNGs in {OUT}',flush=True)

if __name__=='__main__':main()

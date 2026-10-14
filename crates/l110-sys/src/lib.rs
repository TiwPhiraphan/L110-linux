//! l110-sys: คำสั่งกลุ่มระบบ/ข้อมูล (M0: `echo`, `true`, `false`)

mod echo;
mod truefalse;

use l110_core::Applet;

/// applet ทั้งหมดของ crate นี้ (ให้ binary หลักรวมเข้าตารางกลาง)
pub static APPLETS: &[&Applet] = &[&echo::APPLET, &truefalse::TRUE, &truefalse::FALSE];

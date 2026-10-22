// SPDX-License-Identifier: AGPL-3.0-only
// This file is part of Daywerk. See LICENSE for details.

// Prevents an additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    app_lib::run();
}

//! # fo3_viewer
//!
//! Fallout 3 メッシュ & セルシーンビューアー (winit + wgpu)。

mod anim;
mod app;
mod controller;
mod hud;
mod interact;
mod interactive_anim;
mod inventory;
mod loader;
mod types;
mod ui;

use std::env;
use winit::event_loop::EventLoop;

use crate::app::App;
use crate::types::ViewerTarget;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("使用法:");
        println!("  - メッシュ単体表示: cargo run -p fo3_viewer -- <DataDir> <RelativeNifPath>");
        println!("  - セル一括表示:     cargo run -p fo3_viewer -- cell <DataDir> <CellEDID>");
        println!("  - ワールド表示:     cargo run -p fo3_viewer -- world <DataDir> <WorldEDID> [GridX] [GridY]");
        println!("  - アニメーション:   cargo run -p fo3_viewer -- anim <DataDir> <RelativeNifPath> <RelativeKfPath>");
        println!("例:");
        println!("  cargo run -p fo3_viewer -- \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"meshes\\weapons\\1handpistol\\10mmpistol.nif\"");
        println!("  cargo run -p fo3_viewer -- cell \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"Vault101a\"");
        println!("  cargo run -p fo3_viewer -- cell \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"Springvale\"");
        println!("  cargo run -p fo3_viewer -- world \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"MegatonWorld\"");
        println!("  cargo run -p fo3_viewer -- world \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"DCWorld01\"");
        println!("  cargo run -p fo3_viewer -- world \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"Wasteland\" -1 2");
        println!("  cargo run -p fo3_viewer -- anim \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"meshes\\characters\\_male\\upperbody.nif\" \"meshes\\characters\\_male\\idleanims\\ttnpchappysubtlelistena.kf\"");
        return Ok(());
    }

    let (data_dir, target) = if args[1] == "cell" {
        if args.len() < 4 {
            eprintln!("エラー: セル表示モードには <DataDir> と <CellEDID> が必要です。");
            return Ok(());
        }
        (args[2].clone(), ViewerTarget::Cell(args[3].clone()))
    } else if args[1] == "world" {
        if args.len() < 4 {
            eprintln!("エラー: ワールド表示モードには <DataDir> と <WorldEDID> [GridX] [GridY] が必要です。");
            return Ok(());
        }
        let grid = if args.len() >= 6 {
            let gx = args[4].parse::<i32>().unwrap_or(0);
            let gy = args[5].parse::<i32>().unwrap_or(0);
            Some((gx, gy))
        } else {
            None
        };
        (args[2].clone(), ViewerTarget::World(args[3].clone(), grid))
    } else if args[1] == "anim" {
        // anim <DataDir> <NifPath> <KfPath>
        if args.len() < 5 {
            eprintln!("エラー: アニメーションモードには <DataDir> <NifPath> <KfPath> が必要です。");
            eprintln!("例: cargo run -p fo3_viewer -- anim \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"meshes\\characters\\_male\\upperbody.nif\" \"meshes\\characters\\_male\\idleanims\\ttnpchappysubtlelistena.kf\"");
            return Ok(());
        }
        (
            args[2].clone(),
            ViewerTarget::Anim {
                nif_path: args[3].clone(),
                kf_path: args[4].clone(),
            },
        )
    } else if args[1] == "actor" {
        // actor <DataDir> [naked | outfit_path] <KfPath>
        if args.len() < 5 {
            eprintln!("エラー: アクターモードには <DataDir> [naked | outfit_path] <KfPath> が必要です。");
            eprintln!("例 (素体): cargo run -p fo3_viewer -- actor \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" naked \"meshes\\characters\\_male\\idleanims\\ttnpchappysubtlelistena.kf\"");
            eprintln!("例 (防具): cargo run -p fo3_viewer -- actor \"A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\" \"meshes\\armor\\wastelandclothing01\\outfitm.nif\" \"meshes\\characters\\_male\\idleanims\\ttnpchappysubtlelistena.kf\"");
            return Ok(());
        }
        (
            args[2].clone(),
            ViewerTarget::Actor {
                outfit_or_naked: args[3].clone(),
                kf_path: args[4].clone(),
            },
        )
    } else {
        (args[1].clone(), ViewerTarget::Mesh(args[2].clone()))
    };

    let event_loop = EventLoop::new()?;
    let mut app = App {
        data_dir,
        target,
        state: None,
    };

    event_loop.run_app(&mut app)?;

    Ok(())
}

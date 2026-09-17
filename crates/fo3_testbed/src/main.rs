use fo3_bsa::BsaArchive;
use fo3_nif::blocks::*;
use fo3_nif::NifFile;
use fo3_vfs::VfsManager;

fn print_nif_info(vfs: &mut VfsManager, path: &str) {
    println!("\n=== Inspecting: {} ===", path);
    let bytes = match vfs.read(path) {
        Ok(b) => b,
        Err(e) => {
            println!("Failed to read {}: {}", path, e);
            return;
        }
    };

    let nif = match NifFile::read(&mut std::io::Cursor::new(&bytes)) {
        Ok(n) => n,
        Err(e) => {
            println!("Failed to parse {}: {}", path, e);
            return;
        }
    };

    for (i, block) in nif.blocks.iter().enumerate() {
        if let NifBlock::NiTriShape(shape) = block {
            println!(
                "Shape [{}]: name={}",
                i,
                nif.get_string(shape.geom.av.net.name_index as u32)
                    .unwrap_or("")
            );
            for p in &shape.geom.av.properties {
                if *p < 0 {
                    continue;
                }
                if let Some(NifBlock::BSShaderPPLightingProperty(shader)) =
                    nif.blocks.get(*p as usize)
                {
                    if shader.texture_set >= 0 {
                        if let Some(NifBlock::BSShaderTextureSet(tex_set)) =
                            nif.blocks.get(shader.texture_set as usize)
                        {
                            if !tex_set.textures.is_empty() {
                                println!("  - Texture 0: {}", tex_set.textures[0]);
                            }
                        }
                    }
                }
            }
        } else if let NifBlock::NiTriStrips(strips) = block {
            println!(
                "Strips [{}]: name={}",
                i,
                nif.get_string(strips.geom.av.net.name_index as u32)
                    .unwrap_or("")
            );
            for p in &strips.geom.av.properties {
                if *p < 0 {
                    continue;
                }
                if let Some(NifBlock::BSShaderPPLightingProperty(shader)) =
                    nif.blocks.get(*p as usize)
                {
                    if shader.texture_set >= 0 {
                        if let Some(NifBlock::BSShaderTextureSet(tex_set)) =
                            nif.blocks.get(shader.texture_set as usize)
                        {
                            if !tex_set.textures.is_empty() {
                                println!("  - Texture 0: {}", tex_set.textures[0]);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    let mut vfs = VfsManager::new();
    vfs.add_loose_root("A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data");
    if let Ok(bsa) = BsaArchive::open(
        "A:\\SteamLibrary\\steamapps\\common\\Fallout 3 goty\\Data\\Fallout - Meshes.bsa",
    ) {
        vfs.add_bsa(bsa);
    }

    print_nif_info(&mut vfs, "meshes/terminals/terminal01.nif");
    print_nif_info(
        &mut vfs,
        "meshes/animobjects/gene_projector/gene_projector.nif",
    );
    print_nif_info(&mut vfs, "meshes/terminals/geneprojector01.nif");
    print_nif_info(&mut vfs, "meshes/terminals/terminalinterface01.nif");
}

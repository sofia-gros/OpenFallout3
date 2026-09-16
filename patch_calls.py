import sys
import glob

for f in glob.glob('crates/fo3_render/src/scene/*.rs'):
    content = open(f, 'r', encoding='utf-8').read()
    if f.endswith('traversal.rs'):
        continue
    content = content.replace('''traverse_block(
                0,
                &root_transform,
                None,
                None,
                nif,''', '''traverse_block(
                0,
                &root_transform,
                None,
                None,
                None,
                nif,''')
    content = content.replace('''traverse_block(
                0,
                &root_transform,
                None,
                None,
                part_nif,''', '''traverse_block(
                0,
                &root_transform,
                None,
                None,
                None,
                part_nif,''')
    open(f, 'w', encoding='utf-8').write(content)

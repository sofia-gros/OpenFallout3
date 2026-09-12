//! # ESM セル・ワールド空間検索クエリ
//!
//! Fallout 3 (Gamebryo 2.6) の CELL / WRLD レコードおよび子グループを走査し、
//! セル・ワールドスペース・配置オブジェクト (REFR)・地形 (LAND) を検索・抽出する。
//!
//! 参照元:
//! - `references/openmw/components/esm4/loadcell.hpp`
//! - `references/openmw/components/esm4/loadwrld.hpp`
//! - `knowledge/worldspace_cells.md`

use std::collections::HashMap;
use std::io::{self, Read, Seek, SeekFrom};

use crate::header::GroupHeader;
use crate::reader::{EsmEntry, EsmReader};
use crate::records::{CellRecord, DialRecord, InfoRecord, LandRecord, RefrRecord, TermRecord, WorldRecord};
use crate::types::{
    FormId, REC_ACHR, REC_ACRE, REC_CELL, REC_DIAL, REC_INFO, REC_LAND, REC_REFR, REC_TERM,
    REC_WRLD,
};

impl<R: Read + Seek> EsmReader<R> {
    /// 指定された EDID を持つ CELL レコード、その子 REFR レコード群、および地形 LAND レコード（存在する場合）を検索・取得する。
    ///
    /// 内部セル（トップレベル CELL グループ）および外部セル（WRLD グループ配下）の双方を走査する。
    pub fn find_cell_by_edid(
        &mut self,
        target_edid: &str,
    ) -> io::Result<Option<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        // 1. トップレベルの CELL グループ内を走査
        let mut wrld_group_start = 0u64;
        let mut wrld_group_size = 0u64;

        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    let rtype = group.target_record_type();
                    let group_end =
                        self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                    if rtype == Some(REC_CELL) {
                        if let Some(res) = self.search_cell_in_stream(group_end, target_edid)? {
                            return Ok(Some(res));
                        }
                    } else if rtype == Some(REC_WRLD) {
                        wrld_group_start = self.reader.stream_position()?;
                        wrld_group_size = group.group_size as u64 - GroupHeader::SIZE as u64;
                        self.skip(wrld_group_size)?;
                    } else {
                        let rem = group.group_size as u64 - GroupHeader::SIZE as u64;
                        self.skip(rem)?;
                    }
                }
                EsmEntry::Record(rec, _) => {
                    self.skip(rec.data_size as u64)?;
                }
            }
        }

        // 2. WRLD グループ配下の走査（外部セル）
        if wrld_group_start > 0 {
            self.reader.seek(SeekFrom::Start(wrld_group_start))?;
            let wrld_end = wrld_group_start + wrld_group_size;
            return self.search_cell_in_stream(wrld_end, target_edid);
        }

        Ok(None)
    }

    /// 指定ストリーム範囲内から target_edid に一致する CELL およびその子要素を再帰走査する。
    fn search_cell_in_stream(
        &mut self,
        group_end: u64,
        target_edid: &str,
    ) -> io::Result<Option<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)>> {
        let mut target_cell: Option<CellRecord> = None;
        let mut refrs = Vec::new();
        let mut land: Option<LandRecord> = None;

        while self.reader.stream_position()? < group_end {
            let entry = match self.read_next_entry()? {
                Some(e) => e,
                None => break,
            };

            match entry {
                EsmEntry::Group(group) => {
                    let group_size = group.group_size as u64 - GroupHeader::SIZE as u64;
                    let inner_end = self.reader.stream_position()? + group_size;

                    if let Some(ref cell) = target_cell {
                        let cell_id = cell.form_id.0;
                        let group_label_id = u32::from_le_bytes(group.label);
                        if (group.group_type == 6
                            || group.group_type == 8
                            || group.group_type == 9
                            || group.group_type == 10)
                            && group_label_id == cell_id
                        {
                            self.collect_children_in_group(inner_end, &mut refrs, &mut land)?;
                            return Ok(Some((cell.clone(), refrs, land)));
                        }
                    }

                    if group.group_type == 6
                        || group.group_type == 7
                        || group.group_type == 8
                        || group.group_type == 9
                        || group.group_type == 10
                    {
                        self.skip(group_size)?;
                        continue;
                    }

                    if let Some(result) = self.search_cell_in_stream(inner_end, target_edid)? {
                        return Ok(Some(result));
                    }
                }
                EsmEntry::Record(header, subrecords) => {
                    if header.type_id == REC_CELL {
                        let cell = CellRecord::from_record(&header, &subrecords)?;
                        if cell.edid.eq_ignore_ascii_case(target_edid) {
                            target_cell = Some(cell);
                            let pos = self.reader.stream_position()?;
                            if pos < group_end {
                                if let Some(next_entry) = self.read_next_entry()? {
                                    if let EsmEntry::Group(child_group) = next_entry {
                                        let child_size =
                                            child_group.group_size as u64 - GroupHeader::SIZE as u64;
                                        let child_end = self.reader.stream_position()? + child_size;
                                        self.collect_children_in_group(
                                            child_end, &mut refrs, &mut land,
                                        )?;
                                        return Ok(Some((target_cell.unwrap(), refrs, land)));
                                    }
                                }
                            }
                            return Ok(Some((target_cell.unwrap(), Vec::new(), None)));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// グループ内の全 REFR および LAND レコードを収集する。
    fn collect_children_in_group(
        &mut self,
        end_pos: u64,
        refrs: &mut Vec<RefrRecord>,
        land: &mut Option<LandRecord>,
    ) -> io::Result<()> {
        while self.reader.stream_position()? < end_pos {
            let entry = match self.read_next_entry()? {
                Some(e) => e,
                None => break,
            };

            match entry {
                EsmEntry::Group(g) => {
                    let inner_size = g.group_size as u64 - GroupHeader::SIZE as u64;
                    let inner_end = self.reader.stream_position()? + inner_size;
                    self.collect_children_in_group(inner_end, refrs, land)?;
                }
                EsmEntry::Record(header, subrecords) => {
                    if header.type_id == REC_REFR
                        || header.type_id == REC_ACHR
                        || header.type_id == REC_ACRE
                    {
                        let refr = RefrRecord::from_record(&header, &subrecords)?;
                        refrs.push(refr);
                    } else if header.type_id == REC_LAND {
                        if let Ok(l) = LandRecord::parse(&header, &subrecords) {
                            *land = Some(l);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 指定された EDID を持つ WRLD (World Space) レコードを検索し、
    /// (WorldRecord, 子グループ開始位置, 子グループ終了位置) を返す。
    pub fn find_world_by_edid(
        &mut self,
        target_edid: &str,
    ) -> io::Result<Option<(WorldRecord, u64, u64)>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    let rtype = group.target_record_type();
                    let group_end =
                        self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                    if rtype == Some(REC_WRLD) {
                        while self.reader.stream_position()? < group_end {
                            if let Some(inner) = self.read_next_entry()? {
                                match inner {
                                    EsmEntry::Record(rec_hdr, subs) => {
                                        if rec_hdr.type_id == REC_WRLD {
                                            let world =
                                                WorldRecord::from_subrecords(rec_hdr.form_id, &subs);
                                            if world.edid.eq_ignore_ascii_case(target_edid) {
                                                let next_pos = self.reader.stream_position()?;
                                                if let Some(next_entry) = self.read_next_entry()? {
                                                    if let EsmEntry::Group(child_grp) = next_entry {
                                                        let child_start = self.reader.stream_position()?;
                                                        let child_end = child_start
                                                            + (child_grp.group_size as u64
                                                                - GroupHeader::SIZE as u64);
                                                        return Ok(Some((
                                                            world,
                                                            child_start,
                                                            child_end,
                                                        )));
                                                    }
                                                }
                                                self.reader.seek(SeekFrom::Start(next_pos))?;
                                                return Ok(Some((world, 0, 0)));
                                            }
                                        }
                                    }
                                    EsmEntry::Group(g) => {
                                        let skip = g.group_size as u64 - GroupHeader::SIZE as u64;
                                        self.skip(skip)?;
                                    }
                                }
                            }
                        }
                    } else {
                        let rem = group.group_size as u64 - GroupHeader::SIZE as u64;
                        self.skip(rem)?;
                    }
                }
                EsmEntry::Record(rec, _) => {
                    self.skip(rec.data_size as u64)?;
                }
            }
        }
        Ok(None)
    }

    /// ワールドスペースの子グループ内から、指定グリッドを中心とする半径 `radius` マス以内の
    /// 全セル（CELL, REFR, LAND）をストリーム走査で一括抽出する。
    ///
    /// 独立ワールド空間（MegatonWorld 等: 有効セル数 25 件以下）の場合は、
    /// グリッドフィルタによらずパーシステントセルを含む全景セルを一括返却する。
    pub fn read_cells_in_world_region(
        &mut self,
        group_start: u64,
        group_end: u64,
        center_grid: Option<(i32, i32)>,
        radius: i32,
    ) -> io::Result<(
        Option<(i32, i32)>,
        Vec<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)>,
    )> {
        self.reader.seek(SeekFrom::Start(group_start))?;
        let mut cell_order = Vec::new();
        let mut cell_map: HashMap<FormId, (CellRecord, Vec<RefrRecord>, Option<LandRecord>)> =
            HashMap::new();

        self.collect_region_cells_recursive(
            group_end,
            center_grid,
            radius,
            &mut cell_order,
            &mut cell_map,
        )?;

        // 有効セル（REFR または LAND または EDID を保持するセル）のみを抽出
        let mut all_valid_cells: Vec<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)> = cell_order
            .into_iter()
            .filter_map(|id| cell_map.remove(&id))
            .filter(|(cell, refrs, land)| {
                !refrs.is_empty() || land.is_some() || !cell.edid.is_empty()
            })
            .collect();

        // 独立ワールド空間 (MegatonWorld 等: 有効セル数 25 件以下) なら全景（パーシステントセル含む）を一括返却
        if all_valid_cells.len() <= 25 {
            let resolved_center = all_valid_cells
                .iter()
                .max_by_key(|(_, r, _)| r.len())
                .and_then(|(c, _, _)| c.grid);
            return Ok((resolved_center, all_valid_cells));
        }

        if let Some((cx, cy)) = center_grid {
            // 巨大ワールド空間: 指定グリッドの周囲 radius マスに厳密フィルタ
            all_valid_cells.retain(|(cell, _, _)| {
                if let Some((gx, gy)) = cell.grid {
                    (gx - cx).abs() <= radius && (gy - cy).abs() <= radius
                } else {
                    true
                }
            });
            Ok((Some((cx, cy)), all_valid_cells))
        } else {
            // グリッド未指定の場合: 最も配置物 (REFR) が多いセルを中心として radius マスを抽出
            let best_center = all_valid_cells
                .iter()
                .max_by_key(|(_, r, _)| r.len())
                .and_then(|(c, _, _)| c.grid);

            if let Some((cx, cy)) = best_center {
                all_valid_cells.retain(|(cell, _, _)| {
                    if let Some((gx, gy)) = cell.grid {
                        (gx - cx).abs() <= radius && (gy - cy).abs() <= radius
                    } else {
                        true
                    }
                });
                Ok((Some((cx, cy)), all_valid_cells))
            } else {
                Ok((None, all_valid_cells))
            }
        }
    }

    /// ワールドスペース内のグループ階層を再帰走査し、セルとその子要素を収集する。
    fn collect_region_cells_recursive(
        &mut self,
        end_pos: u64,
        center_grid: Option<(i32, i32)>,
        radius: i32,
        cell_order: &mut Vec<FormId>,
        cell_map: &mut HashMap<FormId, (CellRecord, Vec<RefrRecord>, Option<LandRecord>)>,
    ) -> io::Result<()> {
        while self.reader.stream_position()? < end_pos {
            let entry = match self.read_next_entry()? {
                Some(e) => e,
                None => break,
            };

            match entry {
                EsmEntry::Group(group) => {
                    let group_size = group.group_size as u64 - GroupHeader::SIZE as u64;
                    let inner_end = self.reader.stream_position()? + group_size;

                    // Group Type 3: Exterior Cell Block (X, Y block)
                    if group.group_type == 3 {
                        if let Some((cx, cy)) = center_grid {
                            let block_y = i16::from_le_bytes([group.label[0], group.label[1]]) as i32;
                            let block_x = i16::from_le_bytes([group.label[2], group.label[3]]) as i32;
                            let min_gx = block_x * 8;
                            let max_gx = min_gx + 7;
                            let min_gy = block_y * 8;
                            let max_gy = min_gy + 7;
                            if (cx + radius) < min_gx
                                || (cx - radius) > max_gx
                                || (cy + radius) < min_gy
                                || (cy - radius) > max_gy
                            {
                                self.skip(group_size)?;
                                continue;
                            }
                        }
                    }

                    // Group Type 6, 8, 9, 10: Cell Children
                    if group.group_type == 6
                        || group.group_type == 8
                        || group.group_type == 9
                        || group.group_type == 10
                    {
                        let parent_cell_id = FormId(u32::from_le_bytes(group.label));
                        if let Some((_, refrs, land)) = cell_map.get_mut(&parent_cell_id) {
                            self.collect_children_in_group(inner_end, refrs, land)?;
                            continue;
                        }
                    }

                    self.collect_region_cells_recursive(
                        inner_end,
                        center_grid,
                        radius,
                        cell_order,
                        cell_map,
                    )?;
                }
                EsmEntry::Record(header, subrecords) => {
                    if header.type_id == REC_CELL {
                        let cell = CellRecord::from_record(&header, &subrecords)?;
                        let cell_id = cell.form_id;
                        if !cell_map.contains_key(&cell_id) {
                            cell_order.push(cell_id);
                            cell_map.insert(cell_id, (cell, Vec::new(), None));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 指定されたセル名 (EDID) を検索し、そのセルが屋外であれば同一ワールド内の近傍セル（中心セル含め 3×3）を、
    /// 室内セルであれば単一セルを返却する。
    pub fn find_cell_and_neighbors(
        &mut self,
        target_edid: &str,
        radius: i32,
    ) -> io::Result<Option<Vec<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)>>> {
        if let Some((center_cell, center_refrs, center_land)) = self.find_cell_by_edid(target_edid)? {
            if center_cell.is_interior() {
                return Ok(Some(vec![(center_cell, center_refrs, center_land)]));
            }

            if let Some((grid_x, grid_y)) = center_cell.grid {
                if let Some((_world, group_start, group_end)) = self.find_world_by_edid("Wasteland")? {
                    let (_, cells) = self.read_cells_in_world_region(
                        group_start,
                        group_end,
                        Some((grid_x, grid_y)),
                        radius,
                    )?;
                    return Ok(Some(cells));
                }
            }

            Ok(Some(vec![(center_cell, center_refrs, center_land)]))
        } else {
            Ok(None)
        }
    }

    /// 指定された REFR FormID を含むセル、およびそのセルが属するワールドスペース（屋外の場合）を検索して返却する。
    ///
    /// - セルがインテリアセルの場合: `parent_world` は `None`
    /// - セルがワールドスペース（屋外）に属する場合: `parent_world` は `Some(WorldRecord)`
    pub fn find_cell_containing_refr(
        &mut self,
        target_refr_id: FormId,
    ) -> io::Result<Option<(CellRecord, Vec<RefrRecord>, Option<LandRecord>, Option<WorldRecord>)>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        let mut wrld_groups = Vec::new();

        // 1. トップレベル CELL グループの走査（インテリアセル）
        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    let rtype = group.target_record_type();
                    let group_end =
                        self.reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                    if rtype == Some(REC_CELL) {
                        if let Some((cell, refrs, land)) =
                            self.search_cell_with_refr_in_stream(group_end, target_refr_id)?
                        {
                            return Ok(Some((cell, refrs, land, None)));
                        }
                    } else if rtype == Some(REC_WRLD) {
                        let g_start = self.reader.stream_position()?;
                        let g_size = group.group_size as u64 - GroupHeader::SIZE as u64;
                        wrld_groups.push((g_start, g_start + g_size));
                        self.skip(g_size)?;
                    } else {
                        let rem = group.group_size as u64 - GroupHeader::SIZE as u64;
                        self.skip(rem)?;
                    }
                }
                EsmEntry::Record(rec, _) => {
                    self.skip(rec.data_size as u64)?;
                }
            }
        }

        // 2. 各 WRLD グループの走査（外部セル）
        for (w_start, w_end) in wrld_groups {
            self.reader.seek(SeekFrom::Start(w_start))?;
            let mut current_world: Option<WorldRecord> = None;
            while self.reader.stream_position()? < w_end {
                let entry = match self.read_next_entry()? {
                    Some(e) => e,
                    None => break,
                };

                match entry {
                    EsmEntry::Record(rec_hdr, subs) => {
                        if rec_hdr.type_id == REC_WRLD {
                            current_world =
                                Some(WorldRecord::from_subrecords(rec_hdr.form_id, &subs));
                        }
                    }
                    EsmEntry::Group(child_grp) => {
                        let child_end = self.reader.stream_position()?
                            + (child_grp.group_size as u64 - GroupHeader::SIZE as u64);
                        if let Some((cell, refrs, land)) =
                            self.search_cell_with_refr_in_stream(child_end, target_refr_id)?
                        {
                            return Ok(Some((cell, refrs, land, current_world)));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// ストリームから指定 REFR を含む CELL を検索する。
    fn search_cell_with_refr_in_stream(
        &mut self,
        group_end: u64,
        target_refr_id: FormId,
    ) -> io::Result<Option<(CellRecord, Vec<RefrRecord>, Option<LandRecord>)>> {
        let mut current_cell: Option<CellRecord> = None;
        let mut refrs = Vec::new();
        let mut land: Option<LandRecord> = None;

        while self.reader.stream_position()? < group_end {
            let entry = match self.read_next_entry()? {
                Some(e) => e,
                None => break,
            };

            match entry {
                EsmEntry::Group(group) => {
                    let group_size = group.group_size as u64 - GroupHeader::SIZE as u64;
                    let inner_end = self.reader.stream_position()? + group_size;

                    if let Some(ref cell) = current_cell {
                        let cell_id = cell.form_id.0;
                        let group_label_id = u32::from_le_bytes(group.label);
                        if (group.group_type == 6
                            || group.group_type == 8
                            || group.group_type == 9
                            || group.group_type == 10)
                            && group_label_id == cell_id
                        {
                            self.collect_children_in_group(inner_end, &mut refrs, &mut land)?;
                            // 対象の REFR が見つかったか判定
                            if refrs.iter().any(|r| r.form_id == target_refr_id) {
                                return Ok(Some((cell.clone(), refrs, land)));
                            }
                            continue;
                        } else {
                            current_cell = None;
                            refrs.clear();
                            land = None;
                        }
                    }

                    if group.group_type == 6
                        || group.group_type == 7
                        || group.group_type == 8
                        || group.group_type == 9
                        || group.group_type == 10
                    {
                        self.skip(group_size)?;
                        continue;
                    }

                    if let Some(res) =
                        self.search_cell_with_refr_in_stream(inner_end, target_refr_id)?
                    {
                        return Ok(Some(res));
                    }
                }
                EsmEntry::Record(header, subrecords) => {
                    if header.type_id == REC_CELL {
                        if let Some(cell) = current_cell.take() {
                            if refrs.iter().any(|r| r.form_id == target_refr_id) {
                                return Ok(Some((cell, refrs, land)));
                            }
                        }
                        refrs.clear();
                        land = None;
                        let cell = CellRecord::from_record(&header, &subrecords)?;
                        current_cell = Some(cell);
                    }
                }
            }
        }

        if let Some(cell) = current_cell {
            if refrs.iter().any(|r| r.form_id == target_refr_id) {
                return Ok(Some((cell, refrs, land)));
            }
        }

        Ok(None)
    }

    /// 指定された FormID を持つ TERM (Terminal) レコードを検索・取得する。
    ///
    /// 参照元: `references/openmw/components/esm4/loadterm.hpp`, UESP: `Fallout 3: Mod_File_Format/TERM`
    pub fn find_terminal(&mut self, target_form_id: FormId) -> io::Result<Option<TermRecord>> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    let rtype = group.target_record_type();
                    let group_size = group.group_size as u64 - GroupHeader::SIZE as u64;
                    if rtype == Some(REC_TERM) {
                        let group_end = self.reader.stream_position()? + group_size;
                        while self.reader.stream_position()? < group_end {
                            if let Some(inner) = self.read_next_entry()? {
                                match inner {
                                    EsmEntry::Record(rec_hdr, subs) => {
                                        if rec_hdr.type_id == REC_TERM && rec_hdr.form_id == target_form_id {
                                            return TermRecord::parse(&rec_hdr, &subs).map(Some);
                                        }
                                    }
                                    EsmEntry::Group(g) => {
                                        let skip = g.group_size as u64 - GroupHeader::SIZE as u64;
                                        self.skip(skip)?;
                                    }
                                }
                            }
                        }
                    } else {
                        self.skip(group_size)?;
                    }
                }
                EsmEntry::Record(rec, subs) => {
                    if rec.type_id == REC_TERM && rec.form_id == target_form_id {
                        return TermRecord::parse(&rec, &subs).map(Some);
                    }
                }
            }
        }

        Ok(None)
    }

    /// 指定された NPC が持つ会話トピック一覧 (`DIAL`) および初期挨拶 (`GREETING` の `INFO`) を取得する。
    ///
    /// 参照元: `references/openmw/components/esm4/loaddial.hpp`, `loadinfo.hpp`
    pub fn find_npc_dialogue(
        &mut self,
        npc_form_id: FormId,
    ) -> io::Result<(Option<InfoRecord>, Vec<(DialRecord, Vec<InfoRecord>)>)> {
        let start_pos = 24 + self.header_record.data_size as u64;
        self.reader.seek(SeekFrom::Start(start_pos))?;

        let mut greeting_info = None;
        let mut topics = Vec::new();

        while let Some(entry) = self.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    let rtype = group.target_record_type();
                    let group_size = group.group_size as u64 - GroupHeader::SIZE as u64;
                    if rtype == Some(REC_DIAL) {
                        let group_end = self.reader.stream_position()? + group_size;
                        let mut current_dial: Option<DialRecord> = None;
                        let mut current_infos = Vec::new();

                        while self.reader.stream_position()? < group_end {
                            if let Some(inner) = self.read_next_entry()? {
                                match inner {
                                    EsmEntry::Record(rec_hdr, subs) => {
                                        if rec_hdr.type_id == REC_DIAL {
                                            if let Some(dial) = current_dial.take() {
                                                if !current_infos.is_empty() || dial.prompt.is_some() {
                                                    topics.push((dial, current_infos));
                                                }
                                                current_infos = Vec::new();
                                            }
                                            if let Ok(d) = DialRecord::parse(&rec_hdr, &subs) {
                                                current_dial = Some(d);
                                            }
                                        } else if rec_hdr.type_id == REC_INFO {
                                            if let Ok(info) = InfoRecord::parse(&rec_hdr, &subs) {
                                                let is_match = info.speaker_npc.is_none() || info.speaker_npc == Some(npc_form_id);
                                                if is_match {
                                                    if let Some(ref d) = current_dial {
                                                        if d.edid.eq_ignore_ascii_case("GREETING") && greeting_info.is_none() {
                                                            greeting_info = Some(info.clone());
                                                        }
                                                    }
                                                    current_infos.push(info);
                                                }
                                            }
                                        }
                                    }
                                    EsmEntry::Group(g) => {
                                        // サブグループ内の INFO レコードを走査
                                        let sub_size = g.group_size as u64 - GroupHeader::SIZE as u64;
                                        let sub_end = self.reader.stream_position()? + sub_size;
                                        while self.reader.stream_position()? < sub_end {
                                            if let Some(inner_entry) = self.read_next_entry()? {
                                                if let EsmEntry::Record(rec_hdr, subs) = inner_entry {
                                                    if rec_hdr.type_id == REC_INFO {
                                                        if let Ok(info) = InfoRecord::parse(&rec_hdr, &subs) {
                                                            let is_match = info.speaker_npc.is_none() || info.speaker_npc == Some(npc_form_id);
                                                            if is_match {
                                                                if let Some(ref d) = current_dial {
                                                                    if d.edid.eq_ignore_ascii_case("GREETING") && greeting_info.is_none() {
                                                                        greeting_info = Some(info.clone());
                                                                    }
                                                                }
                                                                current_infos.push(info);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(dial) = current_dial {
                            if !current_infos.is_empty() || dial.prompt.is_some() {
                                topics.push((dial, current_infos));
                            }
                        }
                        return Ok((greeting_info, topics));
                    } else {
                        self.skip(group_size)?;
                    }
                }
                EsmEntry::Record(rec, _) => {
                    self.skip(rec.data_size as u64)?;
                }
            }
        }

        Ok((greeting_info, topics))
    }
}

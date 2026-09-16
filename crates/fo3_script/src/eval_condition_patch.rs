    pub fn eval_condition_str(&self, expr: &str) -> bool {
        let clean = expr.trim();
        if clean.is_empty() { return true; }

        let mut paren_depth = 0;
        let mut split_or = None;
        let mut split_and = None;
        let chars: Vec<char> = clean.chars().collect();
        for i in 0..chars.len() {
            if chars[i] == '(' { paren_depth += 1; }
            else if chars[i] == ')' { paren_depth -= 1; }
            else if paren_depth == 0 {
                if i < chars.len() - 1 && chars[i] == '|' && chars[i+1] == '|' {
                    split_or = Some(i);
                } else if i < chars.len() - 1 && chars[i] == '&' && chars[i+1] == '&' {
                    split_and = Some(i);
                }
            }
        }

        if let Some(pos) = split_or {
            return self.eval_condition_str(&clean[..pos]) || self.eval_condition_str(&clean[pos + 2..]);
        }
        if let Some(pos) = split_and {
            return self.eval_condition_str(&clean[..pos]) && self.eval_condition_str(&clean[pos + 2..]);
        }

        let clean = clean.trim_matches(|c| c == '(' || c == ' ' || c == ')').trim();
        if clean.is_empty() { return true; }

        let op_opt = if let Some(pos) = clean.find("==") {
            Some((pos, 2, "=="))
        } else if let Some(pos) = clean.find("!=") {
            Some((pos, 2, "!="))
        } else if let Some(pos) = clean.find("<=") {
            Some((pos, 2, "<="))
        } else if let Some(pos) = clean.find(">=") {
            Some((pos, 2, ">="))
        } else if let Some(pos) = clean.find('<') {
            Some((pos, 1, "<"))
        } else if let Some(pos) = clean.find('>') {
            Some((pos, 1, ">"))
        } else {
            None
        };

        if let Some((pos, len, op)) = op_opt {
            let left_str = clean[..pos].trim();
            let right_str = clean[pos + len..].trim();
            let left_val = self.eval_expr(left_str);
            let right_val = self.eval_expr(right_str);
            match op {
                "==" => (left_val - right_val).abs() < 1e-4,
                "!=" => (left_val - right_val).abs() >= 1e-4,
                "<" => left_val < right_val,
                ">" => left_val > right_val,
                "<=" => left_val <= right_val + 1e-4,
                ">=" => left_val >= right_val - 1e-4,
                _ => true,
            }
        } else {
            self.eval_expr(clean).abs() > 1e-4
        }
    }

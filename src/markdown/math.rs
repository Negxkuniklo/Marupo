#[derive(Debug, Clone, PartialEq)]
pub enum MathNode {
    Row(Vec<MathNode>),
    Text(String),
    Operator(String),
    Frac(Box<MathNode>, Box<MathNode>),
    Sqrt(Box<MathNode>),
    SupSub {
        base: Box<MathNode>,
        sup: Option<Box<MathNode>>,
        sub: Option<Box<MathNode>>,
    },
}

pub fn parse_latex_math(input: &str) -> MathNode {
    let mut chars: Vec<char> = input.trim().chars().collect();
    let mut pos = 0;
    let nodes = parse_sequence(&mut chars, &mut pos, None);
    if nodes.len() == 1 {
        nodes.into_iter().next().unwrap()
    } else {
        MathNode::Row(nodes)
    }
}

fn parse_sequence(chars: &[char], pos: &mut usize, stop_char: Option<char>) -> Vec<MathNode> {
    let mut out: Vec<MathNode> = Vec::new();

    while *pos < chars.len() {
        let c = chars[*pos];
        if Some(c) == stop_char {
            *pos += 1;
            break;
        }
        if c.is_whitespace() {
            *pos += 1;
            continue;
        }

        if c == '^' || c == '_' {
            *pos += 1;
            let script = parse_atom(chars, pos);
            let base = out.pop().unwrap_or_else(|| MathNode::Text(String::new()));
            match base {
                MathNode::SupSub {
                    base: b,
                    mut sup,
                    mut sub,
                } => {
                    if c == '^' {
                        sup = Some(Box::new(script));
                    } else {
                        sub = Some(Box::new(script));
                    }
                    out.push(MathNode::SupSub { base: b, sup, sub });
                }
                other => {
                    let (sup, sub) = if c == '^' {
                        (Some(Box::new(script)), None)
                    } else {
                        (None, Some(Box::new(script)))
                    };
                    out.push(MathNode::SupSub {
                        base: Box::new(other),
                        sup,
                        sub,
                    });
                }
            }
            continue;
        }

        let atom = parse_atom(chars, pos);
        out.push(atom);
    }

    out
}

fn skip_ws(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() && chars[*pos].is_whitespace() {
        *pos += 1;
    }
}

fn parse_atom(chars: &[char], pos: &mut usize) -> MathNode {
    skip_ws(chars, pos);
    if *pos >= chars.len() {
        return MathNode::Text(String::new());
    }

    let c = chars[*pos];
    if c == '{' {
        *pos += 1;
        let seq = parse_sequence(chars, pos, Some('}'));
        if seq.len() == 1 {
            return seq.into_iter().next().unwrap();
        }
        return MathNode::Row(seq);
    }

    if c == '\\' {
        *pos += 1;
        if *pos >= chars.len() {
            return MathNode::Text(String::new());
        }
        if !chars[*pos].is_ascii_alphabetic() {
            let esc = chars[*pos];
            *pos += 1;
            return match esc {
                ',' | ';' | ':' | '!' | ' ' => MathNode::Text(" ".to_string()),
                _ => MathNode::Text(esc.to_string()),
            };
        }
        let start = *pos;
        while *pos < chars.len() && chars[*pos].is_ascii_alphabetic() {
            *pos += 1;
        }
        let cmd: String = chars[start..*pos].iter().collect();
        match cmd.as_str() {
            "frac" | "dfrac" | "tfrac" => {
                let num = parse_atom(chars, pos);
                let den = parse_atom(chars, pos);
                return MathNode::Frac(Box::new(num), Box::new(den));
            }
            "sqrt" => {
                skip_ws(chars, pos);
                if *pos < chars.len() && chars[*pos] == '[' {
                    *pos += 1;
                    let _ = parse_sequence(chars, pos, Some(']'));
                }
                let inner = parse_atom(chars, pos);
                return MathNode::Sqrt(Box::new(inner));
            }
            "text" | "mathrm" | "mathbf" | "mathit" => {
                return parse_atom(chars, pos);
            }
            "left" | "right" => {
                skip_ws(chars, pos);
                if *pos < chars.len() {
                    let delim = chars[*pos];
                    *pos += 1;
                    if delim == '.' {
                        return MathNode::Text(String::new());
                    }
                    return MathNode::Operator(delim.to_string());
                }
                return MathNode::Text(String::new());
            }
            _ => {
                if let Some((sym, is_op)) = latex_command_to_unicode(&cmd) {
                    if is_op {
                        return MathNode::Operator(sym.to_string());
                    } else {
                        return MathNode::Text(sym.to_string());
                    }
                } else {
                    return MathNode::Text(cmd);
                }
            }
        }
    }

    *pos += 1;
    match c {
        '+' | '-' | '=' | '<' | '>' | '/' | '*' => MathNode::Operator(format!(" {} ", c)),
        '(' | ')' | '[' | ']' | ',' | ':' | ';' | '|' => MathNode::Operator(c.to_string()),
        _ => MathNode::Text(c.to_string()),
    }
}

fn latex_command_to_unicode(cmd: &str) -> Option<(&'static str, bool)> {
    match cmd {
        // Lowercase Greek
        "alpha" => Some(("α", false)),
        "beta" => Some(("β", false)),
        "gamma" => Some(("γ", false)),
        "delta" => Some(("δ", false)),
        "epsilon" | "varepsilon" => Some(("ε", false)),
        "zeta" => Some(("ζ", false)),
        "eta" => Some(("η", false)),
        "theta" | "vartheta" => Some(("θ", false)),
        "iota" => Some(("ι", false)),
        "kappa" => Some(("κ", false)),
        "lambda" => Some(("λ", false)),
        "mu" => Some(("μ", false)),
        "nu" => Some(("ν", false)),
        "xi" => Some(("ξ", false)),
        "pi" => Some(("π", false)),
        "rho" => Some(("ρ", false)),
        "sigma" => Some(("σ", false)),
        "tau" => Some(("τ", false)),
        "upsilon" => Some(("υ", false)),
        "phi" | "varphi" => Some(("φ", false)),
        "chi" => Some(("χ", false)),
        "psi" => Some(("ψ", false)),
        "omega" => Some(("ω", false)),

        // Uppercase Greek
        "Gamma" => Some(("Γ", false)),
        "Delta" => Some(("Δ", false)),
        "Theta" => Some(("Θ", false)),
        "Lambda" => Some(("Λ", false)),
        "Xi" => Some(("Ξ", false)),
        "Pi" => Some(("Π", false)),
        "Sigma" => Some(("Σ", false)),
        "Phi" => Some(("Φ", false)),
        "Psi" => Some(("Ψ", false)),
        "Omega" => Some(("Ω", false)),

        // Big operators & symbols
        "sum" => Some(("∑", true)),
        "prod" => Some(("∏", true)),
        "int" => Some(("∫", true)),
        "oint" => Some(("∮", true)),
        "infty" => Some(("∞", false)),
        "partial" => Some(("∂", false)),
        "nabla" => Some(("∇", false)),
        "forall" => Some(("∀", true)),
        "exists" => Some(("∃", true)),

        // Binary operators & relations
        "times" => Some((" × ", true)),
        "div" => Some((" ÷ ", true)),
        "pm" => Some((" ± ", true)),
        "mp" => Some((" ∓ ", true)),
        "cdot" => Some((" · ", true)),
        "leq" | "le" => Some((" ≤ ", true)),
        "geq" | "ge" => Some((" ≥ ", true)),
        "neq" | "ne" => Some((" ≠ ", true)),
        "approx" => Some((" ≈ ", true)),
        "equiv" => Some((" ≡ ", true)),
        "sim" => Some((" ∼ ", true)),
        "in" => Some((" ∈ ", true)),
        "notin" => Some((" ∉ ", true)),
        "subset" => Some((" ⊂ ", true)),
        "subseteq" => Some((" ⊆ ", true)),
        "cup" => Some((" ∪ ", true)),
        "cap" => Some((" ∩ ", true)),
        "to" | "rightarrow" => Some((" → ", true)),
        "leftarrow" => Some((" ← ", true)),
        "Rightarrow" => Some((" ⇒ ", true)),
        "Leftarrow" => Some((" ⇐ ", true)),
        "leftrightarrow" => Some((" ↔ ", true)),
        "iff" | "Leftrightarrow" => Some((" ⇔ ", true)),
        "dots" | "ldots" => Some(("…", false)),
        "cdots" => Some(("⋯", false)),

        // Functions
        "sin" => Some(("sin", true)),
        "cos" => Some(("cos", true)),
        "tan" => Some(("tan", true)),
        "log" => Some(("log", true)),
        "ln" => Some(("ln", true)),
        "exp" => Some(("exp", true)),
        "lim" => Some(("lim", true)),
        "max" => Some(("max", true)),
        "min" => Some(("min", true)),
        _ => None,
    }
}

//! MeCab を[解析器](crate::morph::Analyzer)として使う。
//!
//! <strong>辞書は呼ぶ側が名乗る。</strong> MeCab は自分がどの辞書で動いているかを素直に
//! 教えないので、<strong>内容から当てにいかない</strong>——推測を混ぜれば決定的でなくなる。
//!
//! そして<strong>名乗りが UniDic でなければ、測る側が断る</strong>（[`crate::morph::check`]）。

use std::io::Write;
use std::process::{Command, Stdio};

use crate::morph::{Analyzer, Dictionary, Morpheme};

/// 入力の緩衝の上限。<strong>MeCab がこれ以上を受けない。</strong>
///
/// これを超える 1 行は分割され、行の数が合わなくなる——そのときは
/// [まとめて解析する側](Mecab::analyze_all)が<strong>0 本として返す。</strong> ずれた値を返さない。
pub const MAX_INPUT_BUFFER: usize = 8192 * 640;

/// MeCab を外部の実行ファイルとして呼ぶ。
#[derive(Debug, Clone)]
pub struct Mecab {
    /// 実行ファイルの経路。
    pub program: String,
    /// 辞書の経路。`None` なら MeCab の既定。
    pub dicdir: Option<String>,
    /// 辞書の体系。<strong>呼ぶ側が名乗る。</strong>
    pub dictionary: Dictionary,
    /// 辞書の名前。指紋に入る。
    pub dict_name: String,
    /// 辞書の版。指紋に入る。
    pub dict_version: String,
}

impl Mecab {
    /// UniDic を名乗って作る。
    #[must_use]
    pub fn unidic(
        program: impl Into<String>,
        dicdir: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        let dicdir = dicdir.into();
        Self {
            program: program.into(),
            // <strong>空の経路を `-d ""` として渡さない。</strong> MeCab が起動に失敗し、
            // 形態素 0 になって「除外に掛かった」と見分けられなくなる。
            dicdir: (!dicdir.is_empty()).then_some(dicdir),
            dictionary: Dictionary::UnidicShort,
            dict_name: "UniDic".into(),
            dict_version: version.into(),
        }
    }

    /// UniDic でない辞書で作る。<strong>測る側が断る。</strong>
    #[must_use]
    pub fn other(
        program: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            program: program.into(),
            dicdir: None,
            dictionary: Dictionary::Other,
            dict_name: name.into(),
            dict_version: version.into(),
        }
    }

    /// MeCab を呼んで出力を取る。呼べなければ `None`。
    ///
    /// <strong>行の数だけ EOS が返る。</strong> MeCab は行単位で読む。
    fn run(&self, lines: &[&str]) -> Option<String> {
        let rc = self.rcfile()?;
        let mut cmd = Command::new(&self.program);
        // <strong>出力の形をこちらで決める。</strong> 辞書の `dicrc` に任せない。
        cmd.arg("-r").arg(&rc);
        if let Some(d) = &self.dicdir {
            cmd.arg("-d").arg(d);
        }
        // <strong>入力の緩衝を、いちばん長い行に合わせて広げる。</strong> 既定の 8,192 バイトを
        // 超える行は MeCab が<strong>黙って分割する</strong>——分割されると行の数が合わなくなり、
        // 切り分けが 1 つずれる。長い段落は実際の記事に普通に出る。
        let longest = lines.iter().map(|l| l.len()).max().unwrap_or(0);
        cmd.arg("-b")
            .arg((longest + 8192).clamp(8192, MAX_INPUT_BUFFER).to_string());
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        {
            let stdin = child.stdin.as_mut()?;
            for l in lines {
                stdin.write_all(l.as_bytes()).ok()?;
                stdin.write_all(b"\n").ok()?;
            }
        }
        drop(child.stdin.take());
        let out = child.wait_with_output().ok()?;
        String::from_utf8(out.stdout).ok()
    }

    /// 出力の形を決める設定ファイルを置く。<strong>置いた経路を返す。</strong>
    ///
    /// <strong>辞書の `dicrc` が出力の形を決めてしまう。</strong> UniDic 2.1.2 の `dicrc` は
    /// `output-format-type = unidic` を持ち、素性を <strong>タブ区切りに並べ替えて</strong>出す——
    /// しかも `--node-format` を渡しても、その型が優先されて<strong>黙って無視される</strong>。
    ///
    /// <strong>読むのは素性の配列そのものである。</strong>[8 番目が語彙素](parse_output)という
    /// 位置の話は、並べ替えた出力ではなく配列に対して成り立つ。
    fn rcfile(&self) -> Option<std::path::PathBuf> {
        // <strong>知らない語の素性は短い。</strong> 8 番目を読もうとすると MeCab がそこで死ぬ
        // （`given index is out of range`）——<strong>知らない語は実際の記事に必ず出る。</strong>
        // 語彙素の欄には表層形を置く。UniDic は知らない語に語彙素を持たないので、
        // それが[読み手](parse_output)の当てはめる既定と同じものになる。
        const KNOWN: &str = "%m\\t%f[0],%f[1],%f[2],%f[3],%f[4],%f[5],%f[6],%f[7]\\n";
        const UNKNOWN: &str = "%m\\t%f[0],%f[1],%f[2],%f[3],%f[4],%f[5],,%m\\n";
        let path = std::env::temp_dir().join(format!("kakiburi-mecab-{}.rc", std::process::id()));
        let body = format!(
            "node-format-kakiburi = {KNOWN}\n\
             unk-format-kakiburi = {UNKNOWN}\n\
             bos-format-kakiburi =\n\
             eos-format-kakiburi = EOS\\n\n\
             output-format-type = kakiburi\n"
        );
        std::fs::write(&path, body).ok()?;
        Some(path)
    }
}

impl Analyzer for Mecab {
    fn dictionary(&self) -> Dictionary {
        self.dictionary
    }

    fn dictionary_version(&self) -> (String, String) {
        (self.dict_name.clone(), self.dict_version.clone())
    }

    fn analyze(&self, text: &str) -> Vec<Morpheme> {
        self.analyze_all(&[text]).pop().unwrap_or_default()
    }

    /// <strong>1 つの process でまとめて解析する。</strong>
    ///
    /// node ごとに起こせば、200 MB の辞書を node の数だけ読み直す。
    ///
    /// <strong>行の数で切り分ける。</strong> 印を挟まない——印に選んだ文字が本文に出れば、
    /// そこで切れて<strong>静かにずれる</strong>。行の数は数えれば分かる。
    fn analyze_all(&self, texts: &[&str]) -> Vec<Vec<Morpheme>> {
        let lines: Vec<&str> = texts.iter().flat_map(|t| t.split('\n')).collect();
        let Some(out) = self.run(&lines) else {
            // 呼べなかった。<strong>形態素 0 なので除外に掛かる。</strong>
            return vec![Vec::new(); texts.len()];
        };
        let mut blocks = split_eos(&out);
        if blocks.len() != lines.len() {
            return vec![Vec::new(); texts.len()];
        }
        blocks.reverse();
        texts
            .iter()
            .map(|t| {
                let n = t.split('\n').count();
                let mut ms = Vec::new();
                for _ in 0..n {
                    if let Some(b) = blocks.pop() {
                        ms.extend(parse_output(&b));
                    }
                }
                ms
            })
            .collect()
    }
}

/// 出力を行ごとの塊に切る。<strong>`EOS` が 1 行の終わりである。</strong>
fn split_eos(out: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = String::new();
    for line in out.lines() {
        if line == "EOS" {
            blocks.push(std::mem::take(&mut current));
            continue;
        }
        current.push_str(line);
        current.push('\n');
    }
    blocks
}

/// MeCab の出力を読む。
///
/// 1 行が `表層形\t素性,素性,...`。<strong>素性の並びは辞書ごとに違う。</strong>
///
/// UniDic は 8 番目が語彙素だが、<strong>IPADic の 8 番目は読みである。</strong>
/// 同じ位置を読んで違うものが出るので、<strong>体系を名乗らせるしかない</strong>——
/// 名乗りが違えば、語彙素で引く指標が読みを鍵に引いて 0 件になる。
fn parse_output(out: &str) -> Vec<Morpheme> {
    let mut ms = Vec::new();
    for line in out.lines() {
        if line == "EOS" || line.is_empty() {
            continue;
        }
        let Some((surface, rest)) = line.split_once('\t') else {
            continue;
        };
        let f: Vec<&str> = rest.split(',').collect();
        let pos1 = f.first().copied().unwrap_or("*").to_owned();
        let pos2 = f.get(1).copied().unwrap_or("*").to_owned();
        // UniDic は 8 番目が語彙素。無ければ表層形を使う。
        let lemma = f
            .get(7)
            .filter(|s| !s.is_empty() && **s != "*")
            .map_or_else(|| surface.to_owned(), |s| (*s).to_owned());
        ms.push(Morpheme {
            surface: surface.to_owned(),
            lemma,
            pos1,
            pos2,
        });
    }
    ms
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morph::check;

    #[test]
    fn 辞書を名乗らせる() {
        // MeCab は自分がどの辞書で動いているかを素直に教えない。
        // 内容から当てにいかない——推測を混ぜれば決定的でなくなる。
        let unidic = Mecab::unidic("mecab", "/dic/unidic", "3.1.0");
        assert!(check(&unidic).is_ok());

        let ipadic = Mecab::other("mecab", "IPADic", "2.7.0");
        assert!(check(&ipadic).is_err(), "UniDic でなければ断る");
    }

    #[test]
    fn 辞書の名前と版が指紋に出る() {
        let m = Mecab::unidic("mecab", "/dic/unidic", "3.1.0");
        assert_eq!(m.dictionary_version(), ("UniDic".into(), "3.1.0".into()));
    }

    #[test]
    fn 出力を読む() {
        let out = "これ\t名詞,代名詞,一般,*,*,*,これ,コレ,コレ\n\
                   は\t助詞,係助詞,*,*,*,*,は,ハ,ワ\n\
                   EOS\n";
        let ms = parse_output(out);
        assert_eq!(ms.len(), 2, "EOS は形態素ではない");
        assert_eq!(ms[0].surface, "これ");
        assert_eq!(ms[0].pos1, "名詞");
        assert_eq!(ms[1].pos1, "助詞");
        assert!(ms[1].is_function_word());
    }

    #[test]
    fn 同じ位置を読んでも辞書で違うものが出る() {
        // これが「鍵が合わない」の実体である。IPADic の 8 番目は読みで、
        // UniDic の 8 番目は語彙素。<strong>だから体系を名乗らせるしかない。</strong>
        let ipadic = "これ\t名詞,代名詞,一般,*,*,*,これ,コレ,コレ\nEOS\n";
        let ms = parse_output(ipadic);
        assert_eq!(
            ms[0].lemma, "コレ",
            "IPADic の 8 番目は読みである——語彙素として引けば 0 件になる"
        );

        let unidic = "これ\t代名詞,*,*,*,*,*,コレ,此れ,これ,コレ\nEOS\n";
        let ms = parse_output(unidic);
        assert_eq!(ms[0].lemma, "此れ", "UniDic の 8 番目は語彙素である");
    }

    #[test]
    fn 語彙素が無ければ表層形を使う() {
        let out = "走っ\t動詞,自立,*,*,五段・ラ行,連用タ接続\nEOS\n";
        let ms = parse_output(out);
        assert_eq!(ms[0].lemma, "走っ", "欄が無ければ表層形");
    }

    #[test]
    fn 空の辞書経路は既定として扱う() {
        // `-d ""` を渡すと MeCab が起動に失敗し、形態素 0 になって
        // 「除外に掛かった」と見分けられなくなる。
        let m = Mecab::unidic("mecab", "", "3.1.0");
        assert!(m.dicdir.is_none());
        let m = Mecab::unidic("mecab", "/dic/unidic", "3.1.0");
        assert_eq!(m.dicdir.as_deref(), Some("/dic/unidic"));
    }

    #[test]
    fn 呼べなければ空を返す() {
        // 黙って 0 を返すのではない——<strong>形態素が 0 なら除外に掛かる。</strong>
        let m = Mecab::unidic("/存在しない/mecab", "/dic", "3.1.0");
        assert!(m.analyze("これは日本語である。").is_empty());
    }

    #[test]
    fn 呼べなくても本数は合わせる() {
        // 減らして返せば、node を跨がないはずの指標が別の node の形態素を数える。
        let m = Mecab::unidic("/存在しない/mecab", "/dic", "3.1.0");
        assert_eq!(m.analyze_all(&["あ", "い", "う"]).len(), 3);
    }

    #[test]
    fn 出力は_eos_で切る() {
        let out = "あ\t名詞\nEOS\nい\t助詞\nう\t名詞\nEOS\n";
        let blocks = split_eos(out);
        assert_eq!(blocks.len(), 2);
        assert_eq!(parse_output(&blocks[0]).len(), 1);
        assert_eq!(parse_output(&blocks[1]).len(), 2);
    }

    #[test]
    fn 改行を含む_node_は行の数で戻す() {
        // 印を挟まない。印に選んだ文字が本文に出れば、そこで切れて静かにずれる。
        let out = "あ\t名詞\nEOS\nい\t名詞\nEOS\nう\t名詞\nEOS\n";
        let blocks = split_eos(out);
        assert_eq!(blocks.len(), 3, "3 行なので 3 つ");
        // "あ\nい" は 2 行、"う" は 1 行——合わせて 3 行である。
        let texts = ["あ\nい", "う"];
        assert_eq!(
            texts.iter().map(|t| t.split('\n').count()).sum::<usize>(),
            blocks.len()
        );
    }
}

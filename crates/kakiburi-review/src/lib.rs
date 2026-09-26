//! 検め。3 段の判定と、3〜4 本の指摘。
//!
//! 目盛りを作る側に依存しない。 依存すると、検める文書を見てから重みや語彙を
//! 作り直せる経路が開く。目盛りは作り終えた形で渡ってくるものであって、ここで
//! 作るものではない。
//!
//! この境界は `Cargo.toml` が守っている——`kakiburi-scale` を依存に持たない。

pub mod point;
pub mod range;
pub mod verdict;

pub use point::{Point, PointError, Remedies};
pub use range::{appearance_size, Lower, Outside, Range, APPEARANCE_FLOOR};
pub use verdict::{
    judge, judge_with, pick_points, Inspected, Notes, Observed, Outcome, Side, Stage, Verdict,
    MAX_POINTS,
};

/// 検めた結果。3 値と指摘を返す。
#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    /// 判定と、止まった段と理由。
    pub outcome: Outcome,
    /// 指摘。最大 4 本。
    pub points: Vec<Point>,
    /// 照合の直し方。書きぶりの枠を奪わない。
    ///
    /// 系統そのものは指示にならない——「何番目かの次元を増やせ」は言葉にならない。
    /// だが次元が語として読める系統なら、その 1 次元は指示になる。
    pub matching: Vec<String>,
    /// 人らしさの直し方。書きぶりの枠を奪わない。
    ///
    /// 3〜4 本という上限は書きぶりの側の話であり、人らしさはそこに数えない
    /// （[指標](../../../docs/spec/100-metrics.md#指標の種類)）。
    /// 枠を奪い合わせると、機械臭さを消す指示と、その人へ寄せる指示が、席を
    /// 取り合う。
    pub humanness: Vec<String>,
    /// 本人の癖から外れているところ。書きぶりの枠を奪わない。
    ///
    /// 判定には使わない。 実測で、この軸まで判定に入れるとカセットから抜いた
    /// 本人の記事 16 本のうち幅の外に出るものが 2 本から 6 本に増えた。
    pub habits: Vec<Point>,
    /// 使われていない型。書きぶりの枠を奪わない。
    ///
    /// 分布では言えないものがある。 その人が繰り返し使う語の並びは、頻度の
    /// ベクトルに均されて消える——[照合値](Self::matching)がどれだけ寄っても、
    /// その人の型が 1 つも出てこない文章はありうる。
    ///
    /// 判定には使わない。 実測で、本人の記事 50 本のうち 5 本が型を 1 つも
    /// 使っていなかった。止める材料にはできない。
    pub katas: Vec<String>,
    /// 残っている機械の型。書きぶりの枠を奪わない。
    ///
    /// 片側だけでは足りない。[使われていない型](Self::katas)は「本人のものが
    /// 入っていない」ことしか言えず、機械の言い回しが残っていることは言えない。
    ///
    /// 判定には使わない。 基準がよく使う並びでも、本人が偶然そう書くことは
    /// ありうる——止める材料にはできない。
    pub machine_katas: Vec<String>,
    /// 残っている機械の語。書きぶりの枠を奪わない。
    ///
    /// [機械の型](Self::machine_katas)が表層の並びで取りこぼすものを言う——
    /// 同じ癖が語形ごとに割れると、どの綴りも床を割って一度も出てこない。
    ///
    /// 判定には使わない。 型と同じ理由である。
    pub machine_gois: Vec<String>,
    /// 本人の言い回しを使いすぎている箇所。書きぶりの枠を奪わない。
    ///
    /// 「繰り返せ」と言う指摘には上限が要る——言わなければ、受け取った側は
    /// 本人の何倍も入れる。日本語は壊れないので、検査では止まらない。
    ///
    /// 判定には使わない。 その人がたまたま多く使う 1 本はありうる。
    pub overused_katas: Vec<String>,
    /// 本人と違う一人称を選んでいる箇所。書きぶりの枠を奪わない。
    ///
    /// [型](Self::katas)にも[機械の語](Self::machine_gois)にも載らない。
    /// 型は枠の数だけ採るので、実測で `僕は` は割合 0.35 で枠から落ちた。
    /// 機械の語は基準の側から選ぶので、基準がほとんど使わない一人称は挙がらない
    /// ——実測で、基準 44 本のうち `私` は 3 本（7%）で床を割っていた。
    ///
    /// 判定には使わない。 その題材でだけ別の一人称を選ぶことはありうる。
    pub first_person: Vec<String>,
    /// 本人と違う書き出しで始めている、ということ。書きぶりの枠を奪わない。
    ///
    /// 密度でも語の位置でも言えない。 見出しの密度は 1,000 字あたりの本数なので、
    /// 見出しが 1 番目にあっても 3 番目にあっても同じ値になる。型の位置は語の位置で、
    /// 挨拶の前に見出しを 1 本挟んでも 0.000 が 0.02 になるだけである。
    ///
    /// 判定には使わない。 その題材でだけ別の始め方をすることはありうる。
    pub opening: Vec<String>,
}

impl Review {
    /// 判定に使っていない知らせが 1 つでもあるか。
    ///
    /// 見せる側が数え直さないために置く。 数え直すと、判定に使わない口を
    /// 足したときに一覧が片方だけ古くなり、止めない知らせが止める指摘の顔で
    /// 並ぶ。
    #[must_use]
    pub fn has_aside(&self) -> bool {
        !self.habits.is_empty()
            || !self.katas.is_empty()
            || !self.machine_katas.is_empty()
            || !self.machine_gois.is_empty()
            || !self.overused_katas.is_empty()
            || !self.first_person.is_empty()
            || !self.opening.is_empty()
    }
}

/// 機械の語 1 つ。この文章に出ているものだけを渡す。
///
/// [型](Kata)と別に持つのは、照らし方が違うからである——型は書かれた文字列を
/// そのまま探すが、語は解析してから語彙素で照らす。 そうしないと
/// `地味な` と `地味に` が別のものになる。
#[derive(Debug, Clone, PartialEq)]
pub struct Goi {
    /// 語彙素。
    pub text: String,
    /// 基準の単位のうち、これが現れた割合。
    pub rate: f64,
    /// 本人の単位のうち、これが現れた割合。
    ///
    /// 0 とは限らない。 選ぶ条件は本人側の割合に上限を置いているだけなので、
    /// 本人が時々使う語もここに来る——「本人は使わない」と言い切れば、
    /// 受け取った側は自分の文章に現に在る語を無いと言われる。
    pub base: f64,
    /// この文章に現れた回数。
    pub times: usize,
    /// 本人が同じ品詞でよく使う語。置き換える先である。
    ///
    /// 「別の言い方にする」だけでは直せない。 受け取った側は道具の外で語を
    /// 探すことになり、そこで選んだ語がまた本人の使わない語でありうる。
    pub theirs: Vec<String>,
}

/// その人の型 1 つ。
#[derive(Debug, Clone, PartialEq)]
pub struct Kata {
    /// 語の並び。穴あきなら、間を `〜` で見せた形。
    pub text: String,
    /// 本人の単位のうち、これが現れた割合。
    pub rate: f64,
    /// 文書の中での位置の中央。0 に近ければ書き出しの型である。
    pub at: f64,
    /// この文章に現れているか。
    pub used: bool,
    /// この文章の、直す場所。 現れた箇所の前後。
    ///
    /// 「言い換えろ」と言うなら、どこを言い換えるのかを言う。 並びだけを渡せば、
    /// 受け取った側が道具の外で探し直すことになる——
    /// [照合の指摘](MatchingObserved::spots)と同じ理由である。
    pub spots: Vec<String>,
    /// この文章に現れた回数。
    ///
    /// 繰り返し出ているものを先に出す。 基準での割合だけで並べると、
    /// 誰でも書く並びが、その機械の癖を押し出す——実測で、基準の 44% が使う
    /// 「ことがあり」が、11% しか使わないが草稿では 4 回出ている「地味に」を
    /// 隠していた。癖は 1 度ではなく何度も出る。
    pub times: usize,
    /// この文章での、日本語 1,000 字あたりの回数。
    pub density: f64,
    /// 本人が 1 本の中で使う、日本語 1,000 字あたりの最大。
    pub ceiling: f64,
    /// 本人の単位のうち、これが現れた割合。
    ///
    /// 0 とは限らない。 機械の型を選ぶ条件は本人側の割合に上限を置いて
    /// いるだけなので、本人が時々使う並びもここに来る。
    pub base: f64,
}

/// 本人の言い回しを使いすぎていると言う、本人の上限に対する倍率。暫定値である。
///
/// 1.0 は[幅の外](Range)と同じ扱いである——本人が一度も書かなかった濃さなら
/// 外である、とする。指示できる指標が本人の幅で見るのと揃えてある。
///
/// 2.0 では捕まらなかった。 実測で、本人の上限 1.9 に対し使いすぎた草稿が
/// 2.6——超えてはいるが 2 倍には遠い。
///
/// > 誤報の率は確かめられていない。 上限は本人の全単位から作るので、
/// > そのうちの 1 本を当てても原理的に超えない——42 本で 0 本という結果は
/// > 循環していて、証拠にならない。本人の held-out が要る。
pub const OVERUSE: f64 = 1.0;

/// 本人の言い回しを使いすぎている箇所。
///
/// 「繰り返せ」には上限が要る。[人らしさの直し方](Review::humanness)は
/// 「その人が現に繰り返している言い回しを、そのまま繰り返す」と言うが、
/// どこまで繰り返してよいかを言わない。 受け取った側は本人の何倍も入れる
/// ——実測で、本人が 42 本で 25 回しか使わない `ことができます` が、直した 1 本に
/// 10 回入った。
///
/// 日本語は壊れないので[検査](Inspected)では止まらない。 本人の頻度と
/// 比べるしかない。
fn overused_katas(katas: &[Kata]) -> Vec<String> {
    let mut over = over_used(katas);
    over.sort_by(|a, b| {
        (b.density / b.ceiling)
            .partial_cmp(&(a.density / a.ceiling))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.text.cmp(&b.text))
    });
    over.into_iter()
        .take(MAX_POINTS)
        .map(|k| {
            format!(
                "「{}」を {} 回。本人は多くても 1,000 字あたり {:.1} 回で、この文章は {:.1} 回。減らす。",
                k.text, k.times, k.ceiling, k.density
            )
        })
        .collect()
}

/// 本人の上限を超えて使っている型。
///
/// 「減らせ」と「繰り返せ」が同じ言い回しに出ないよう、1 箇所で決める。
/// 分かれていたときは、同じ言い回しに増やせと減らせが同時に出ていた。
///
/// 上限 0 のものは言わない。 ここへ来る一覧には、道具が勧めた言い回しだけで
/// なく[草稿が繰り返している並び](Kata)も入っており、その多くは題材の語である
/// ——本人が使わない語をすべて挙げれば、指摘が題材で埋まる。
/// 本人が使わない言い回しのうち機械が使うものは、
/// [機械の型と機械の語](Review::machine_katas)が別に名指しする。
fn over_used(katas: &[Kata]) -> Vec<&Kata> {
    katas
        .iter()
        .filter(|k| k.ceiling > 0.0 && k.times > 1 && k.density > k.ceiling * OVERUSE)
        .collect()
}

/// 使われていない型の渡し方。
///
/// どこで使うかまで言う。 位置が偏っている型は、そこで使うから型なのであって、
/// どこかに混ぜればよいものではない。
fn kata_remedies(katas: &[Kata]) -> Vec<String> {
    let mut missing: Vec<&Kata> = katas.iter().filter(|k| !k.used).collect();
    missing.sort_by(|a, b| {
        b.rate
            .partial_cmp(&a.rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.text.cmp(&b.text))
    });
    missing
        .into_iter()
        .take(MAX_POINTS)
        .map(|k| {
            let at = if k.at < 0.2 {
                "書き出しで"
            } else if k.at > 0.8 {
                "結びで"
            } else {
                ""
            };
            format!(
                "本人は{at}「{}」と書く（{:.0}% の記事で使っている）。この文章には出てこない。",
                k.text,
                k.rate * 100.0
            )
        })
        .collect()
}

/// 残っている機械の語の渡し方。
///
/// [機械の型](machine_kata_remedies)が取りこぼすものを言う。 型は表層の並びを
/// そのまま照合するので、同じ癖が語形ごとに割れると、どの綴りも床を割る——実測で、
/// 基準の池 44 本のうち `地味` は 9 本（20%）に出るのに、`地味に` という綴りは
/// 2 本にしかなく、並びとしては一度も指摘に出せなかった。
///
/// 箇所は言わない。 語形が変わるので、語彙素をそのまま探しても当たらない
/// ——`地味` を探しても `地味な` の前半に当たるだけで、直す場所として渡す意味が無い。
fn machine_goi_remedies(gois: &[Goi]) -> Vec<String> {
    let mut used: Vec<&Goi> = gois.iter().filter(|g| g.times > 0).collect();
    // 草稿で繰り返している順。[機械の型](machine_kata_remedies)と揃える。
    used.sort_by(|a, b| {
        b.times.cmp(&a.times).then_with(|| {
            b.rate
                .partial_cmp(&a.rate)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.text.cmp(&b.text))
        })
    });
    used.into_iter()
        .take(MAX_POINTS)
        .map(|g| {
            // 置き換える先まで言う。 言わなければ、受け取った側が道具の外で
            // 語を探すことになり、そこで選んだ語がまた本人の使わない語でありうる。
            let instead = if g.theirs.is_empty() {
                String::new()
            } else {
                format!(
                    " 本人が同じ品詞でよく使うのは「{}」。",
                    g.theirs.join("」「")
                )
            };
            format!(
                "「{}」をこの文章に {} 回。基準の {:.0}% が使い、{}。{instead}",
                g.text,
                g.times,
                g.rate * 100.0,
                person_side(g.base)
            )
        })
        .collect()
}

/// 本人の側をどう言うか。使わないのか、たまに使うのか。
///
/// 言い切らない。 選ぶ条件は本人側の割合に上限を置いているだけで、
/// 0 を求めていない——言い切れば、自分の文章に現に在る語を無いと言われる。
fn person_side(base: f64) -> String {
    if base <= 0.0 {
        "本人は使わない".to_owned()
    } else {
        format!("本人は {:.0}% でしか使わない", base * 100.0)
    }
}

/// 残っている機械の型の渡し方。
///
/// 使われている側を渡す。[その人の型](kata_remedies)とは向きが逆で、
/// あちらは入っていないものを、こちらは入っているものを言う。
fn machine_kata_remedies(katas: &[Kata]) -> Vec<String> {
    let mut used: Vec<&Kata> = katas.iter().filter(|k| k.used).collect();
    // 草稿で繰り返している順。 基準での割合は同点の決め手にしか使わない。
    used.sort_by(|a, b| {
        b.times.cmp(&a.times).then_with(|| {
            b.rate
                .partial_cmp(&a.rate)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.text.cmp(&b.text))
        })
    });
    used.into_iter()
        .take(MAX_POINTS)
        .map(|k| {
            let where_ = if k.spots.is_empty() {
                String::new()
            } else {
                format!(" 出ている箇所: {}", k.spots.join("／"))
            };
            format!(
                "「{}」がこの文章に {} 回。基準の {:.0}% が使い、{}。言い換える。{where_}",
                k.text,
                k.times,
                k.rate * 100.0,
                person_side(k.base)
            )
        })
        .collect()
}

/// 照合の、次元ごとの観測。
///
/// 照合値は 1 つの数なので、どこが違うのかを言えない。 帯の中で止まったときに
/// 何も出さなければ、受け取った側は動きようがない。
#[derive(Debug, Clone, PartialEq)]
pub struct MatchingObserved {
    /// どの系統か。
    pub system: String,
    /// どの次元か。語・記号・字種など、読める形である。
    pub dim: String,
    /// この文章の、標準化した値。
    pub mine: f64,
    /// 相手集合の代表値。
    pub theirs: f64,
    /// 直したときに照合値が動く量。見込みではなく、そのまま動く量である。
    pub effect: f64,
    /// 本人がその次元をどう書いているかの実例。
    ///
    /// 「増やせ」と言うだけでは、どこに置くのかが分からない。
    pub examples: Vec<String>,
    /// この文章の、直す場所。
    ///
    /// 「減らせ」と言うなら、どれを減らすのかを言う。 本人の実例だけでは、
    /// 自分の文章のどこを直すのかが分からない——実測で、受け取った側が
    /// 道具の外で数え直すことになった。
    pub spots: Vec<String>,
}

/// 人らしさの、指標ごとの観測。正が人の側、負が機械の側。
///
/// 合算した 1 つの値では直し方を渡せない。「機械の側にある」としか言えず、
/// どこをどうすればよいかが出てこない。
#[derive(Debug, Clone, PartialEq)]
pub struct HumannessObserved {
    /// 指標の名前。
    pub name: String,
    /// その指標だけで見た人らしさ値。
    pub value: f64,
    /// 人へ寄せる向き。 `true` なら値を上げる。
    ///
    /// 較正が決める。 定義に固定すると、素材がその向きを支えていない
    /// カセットで直し方に従うほど人らしさが下がる。
    pub raise: bool,
    /// その人が現に繰り返している言い回し。この指標に添えるものだけ。
    ///
    /// 数値と向きだけでは直せない。「その人が繰り返している言い回しを
    /// 繰り返す」と言うなら、その言い回しを渡さなければ、受け取った側は自分で
    /// でっち上げた定型句を挿し込むことになる。
    pub phrases: Vec<String>,
    /// この文章が繰り返しすぎている言い回し。減らす側でだけ意味を持つ。
    ///
    /// 「減らせ」と言うなら、どれを減らすのかを言わなければ直せない。
    pub overused: Vec<String>,
    /// この文章で一度しか出てこない語。
    ///
    /// 「語を散らすな」と言うなら、どれが散らしているのかを言わなければ直せない。
    /// 実測では、この指示を受けた側が散らす方向へ直してしまった。
    pub once_only: Vec<String>,
    /// 直したときに人らしさ値が動く量。
    ///
    /// 正反対を指す直し方が同時に出ることがある。 語彙を散らせと言う指標と、
    /// 言い換えるなと言う指標が同時に出たとき、どちらが勝つかは動く量でしか
    /// 言えない——言わなければ、受け取った側は逆を選ぶ。
    pub effect: f64,
    /// 本人の代表値。比べる相手はここである。
    ///
    /// 0 と比べてはいけない。 0 は人と機械の境目であって、その人のところ
    /// ではない——どの指標も境目より人の側にいるのに、合算では機械の側という
    /// ことが実際に起きる。
    pub target: f64,
}

/// 検めに渡す観測ぜんぶ。
///
/// ばらばらに渡さない。 段の数だけ引数が増えると、呼ぶ側が並びを間違えても
/// 型が合ってしまう。
pub struct Observations<'a> {
    /// [検査](Inspected)の観測。0 段目。
    pub inspections: &'a [Inspected],
    /// 人らしさ値がどちら側か。1 段目。
    pub humanness: Option<Side>,
    /// 照合値がどちら側か。2 段目。
    pub matching: Option<Side>,
    /// 照合値を代わりの値で出したなら、欠けていた系統。
    ///
    /// そのとき照合値は人の側に出たときにしか使わない（[`judge_with`]）。
    pub matching_substituted: Option<&'a str>,
    /// 素材の下限に届かない短さ。届いていれば `None`。
    ///
    /// 値が出なかった段の理由に使う（[`Notes::too_short`]）。
    pub too_short: Option<&'a str>,
    /// 前に出す指標の観測。3 段目。
    pub directives: &'a [Observed],
    /// 効く指標はあったが、本人が全部を無効にしたために前に出す指標が空になったか。
    pub directives_muted: bool,
    /// 一貫しているだけの指標の観測。判定には使わない。
    ///
    /// [条件 2](kakiburi_scale::effective::Effective::narrow_only)で捨てられた軸である
    /// ——基準と本人が一致していても、草稿がそこから外れることはある。
    pub habits: &'a [Observed],
    /// 指標ごとの人らしさ値。1 段目の直し方を組む。
    pub humanness_by_metric: &'a [HumannessObserved],
    /// 相手集合から離れている次元。2 段目の直し方を組む。
    pub diverging: &'a [MatchingObserved],
    /// その人の型と、この文章で使われているか。
    pub katas: &'a [Kata],
    /// 機械の型と、この文章で使われているか。
    pub machine_katas: &'a [Kata],
    /// 機械の語と、この文章に出てきた回数。語彙素で照らしたもの。
    pub machine_gois: &'a [Goi],
    /// 直し方に載せた言い回しと、この文章での使われ方。
    ///
    /// 型とは別に渡す。 使いすぎが起きるのは道具自身が「繰り返せ」と
    /// 名指しした言い回しであって、型ではない——型は本人にしか出ないものなので、
    /// そもそも受け取った側が知らない。
    pub phrases: &'a [Kata],
    /// 本人の一人称と、それが現れた単位の割合。多い順。
    pub first_person: &'a [(String, f64)],
    /// この文章に出てきた一人称と、その回数。
    pub draft_first_person: &'a [(String, usize)],
    /// 本人の書き出しの種類と、その割合。多い順。
    pub opening: &'a [(String, f64)],
    /// この文章の書き出しの種類。node が 1 つも無ければ `None`。
    pub draft_opening: Option<&'a str>,
}

/// 本人がその始め方を選んでいると言える、単位の割合。暫定値である。
///
/// [一人称](FIRST_PERSON_MIN)と同じ形である。 1 本でそう始めただけのものを
/// 「本人はこう始める」と言えば、書き手は一度きりの形に寄せることになる。
pub const OPENING_MIN: f64 = 0.10;

/// 本人が選んでいないと言える、いちばん多い始め方に対する割合。暫定値である。
pub const OPENING_SHARE: f64 = 0.5;

/// 本人と違う書き出しで始めていることを言う。
///
/// この文章が何かで始まっているときだけ見る。 node を 1 つも持たない文章に
/// 始め方は無い。
fn opening_remedies(person: &[(String, f64)], draft: Option<&str>) -> Vec<String> {
    let (Some((top, top_rate)), Some(here)) = (person.first(), draft) else {
        return Vec::new();
    };
    if *top_rate < OPENING_MIN || here == top {
        return Vec::new();
    }
    let mine = person
        .iter()
        .find(|(k, _)| k == here)
        .map_or(0.0, |(_, r)| *r);
    if mine >= top_rate * OPENING_SHARE {
        return Vec::new();
    }
    let theirs = if mine <= 0.0 {
        "本人はそう始めない".to_owned()
    } else {
        format!("本人は {:.0}% の記事でしかそう始めない", mine * 100.0)
    };
    vec![format!(
        "この文章は{here}で始まっている。{theirs}——{:.0}% の記事を{top}で書き始める。",
        top_rate * 100.0
    )]
}

/// 本人がその一人称を選んでいると言える、単位の割合。暫定値である。
///
/// 選ばれた側にだけ掛ける。 1 本で使っただけの一人称を「本人はこう書く」と
/// 言えば、書き手は自分が一度しか書かなかった語に寄せることになる。
pub const FIRST_PERSON_MIN: f64 = 0.10;

/// 本人が選んでいないと言える、いちばん多い一人称に対する割合。暫定値である。
///
/// 本人が使わない一人称だけを言うのでは狭すぎる。 実測で、本人は `僕` を
/// 49 本中 28 本、`私` を 5 本で使っていた——`私` を使うことはあるが、
/// 選んでいるのは `僕` である。
pub const FIRST_PERSON_SHARE: f64 = 0.5;

/// 本人と違う一人称を選んでいることを言う。
///
/// この文章が選んだものだけを見る。 一人称が 1 つも出てこない文章に
/// 「本人は `僕` と書く」と言わない——主語を置かない文章はふつうにあり、
/// 実測で本人自身が 49 本中 21 本でどの一人称も使っていない。
fn first_person_remedies(person: &[(String, f64)], draft: &[(String, usize)]) -> Vec<String> {
    let Some((top, top_rate)) = person.first() else {
        return Vec::new();
    };
    if *top_rate < FIRST_PERSON_MIN {
        return Vec::new();
    }
    let rate_of = |name: &str| -> f64 {
        person
            .iter()
            .find(|(n, _)| n == name)
            .map_or(0.0, |(_, r)| *r)
    };
    let mut used: Vec<&(String, usize)> = draft
        .iter()
        .filter(|(name, times)| {
            *times > 0 && name != top && rate_of(name) < top_rate * FIRST_PERSON_SHARE
        })
        .collect();
    // 多く出ている順。同じなら名前の順。 決めておかないと並びが実装で変わる。
    used.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    used.into_iter()
        .map(|(name, times)| {
            let mine = rate_of(name);
            let theirs = if mine <= 0.0 {
                "本人は使わない".to_owned()
            } else {
                format!("本人は {:.0}% の記事でしか使わない", mine * 100.0)
            };
            format!(
                "「{name}」をこの文章に {times} 回。{theirs}。本人は「{top}」と書く（{:.0}% の記事で使っている）。",
                top_rate * 100.0
            )
        })
        .collect()
}

/// 検める。
///
/// 指摘は判定と同じ集合から取る。 別々に定めれば、止めた理由が指摘に出てこない
/// という食い違いが起きる。
#[must_use]
pub fn review(o: &Observations<'_>, remedies: &dyn Remedies) -> Review {
    let (humanness_by_metric, diverging, directives) =
        (o.humanness_by_metric, o.diverging, o.directives);
    let outcome = judge_with(
        o.inspections,
        o.humanness,
        o.matching,
        Notes {
            substituted: o.matching_substituted,
            too_short: o.too_short,
            directives_muted: o.directives_muted,
        },
        directives,
    );
    // 3 段目まで進んだときだけ書きぶりの指摘を組む。 前の段で止まったなら、
    // 出しても受け取った側は逆向きの直しをする。
    let points = if outcome.stage == Stage::Directive {
        pick_points(directives)
            .into_iter()
            .filter_map(|(o, loc)| point::build(o, loc, remedies).ok())
            .collect()
    } else {
        Vec::new()
    };
    // 1 段目で止まったときにこそ、人らしさの直し方を渡す。 ここで何も出さな
    // ければ、機械の書いた草稿は止められるだけで直せない——道具が使われる
    // 場面で出力が空になる。
    //
    // 帯の中で止まったときも渡す。 機械の側に落ちたときだけ出すと、
    // 帯の中から人の側へ出る道が示されない——通らないよりも判定できないの
    // ほうが多いので、そちらで黙るほうが害が大きい。
    // 上限を超えている言い回しは「繰り返せ」の一覧から外す。 外さないと、
    // 同じ言い回しに増やせと減らせが同時に出て、受け取った側はどちらに従っても
    // 片方の指摘に背く。実測では減らした結果、人らしさ値が 0.788 から 0.484 へ下がった。
    let ceiling_hit: Vec<&str> = over_used(o.phrases)
        .iter()
        .map(|k| k.text.as_str())
        .collect();
    let humanness = if outcome.stage == Stage::Humanness {
        humanness_remedies(humanness_by_metric, remedies, &ceiling_hit)
    } else {
        Vec::new()
    };
    // 2 段目で止まったときにも渡す。 ここで何も出さなければ、その人へ寄せる
    // という目的そのものに対して、道具が黙ることになる。
    let matching = if outcome.stage == Stage::Matching {
        matching_remedies(diverging)
    } else {
        Vec::new()
    };
    // どの段で止まっても渡す。 型はその人へ寄せるためのもので、
    // 寄せることが目的そのものである（[軸](../../../docs/spec/000-axis.md#やりたいこと)）。
    //
    // 人らしさの段で黙ってはいけない。 あの段が見ているのは
    // 読みづらさの元が残っているかであって、目的ではない——そこで型を伏せると、
    // 目的の側の言葉が 1 つも出ないまま止まる。
    //
    // 向きも衝突しない。 人らしさは「同じ言い回しを繰り返せ」と言い、型は
    // 「本人はこう書く」と言う。型を使えば繰り返しも増える。
    //
    // 通ったときにも渡す。 分布が寄っていても型が 1 つも出てこないことは
    // ありうる——そこで黙れば、道具は「通った」としか言わないまま
    // その人らしくない文章を返す。
    let katas = kata_remedies(o.katas);
    // こちらは人らしさで止まったときにも出す。 機械の言い回しが残っている
    // ことは、機械の側で止まった理由そのものでありうる。
    let machine_katas = machine_kata_remedies(o.machine_katas);
    // 並びで割れた癖を、語彙素で拾い直す。
    let machine_gois = machine_goi_remedies(o.machine_gois);
    // 本人の型を使いすぎていないか。 寄せろと言った側が行きすぎるのを止める。
    let overused_katas = overused_katas(o.phrases);
    // その人へ寄せるための知らせなので、人らしさの段では出さない。 型は全段で出すが、
    // 癖は照合値の段と指示できる指標の段でだけ出す。
    let habits = if matches!(outcome.stage, Stage::Matching | Stage::Directive) {
        pick_points(o.habits)
            .into_iter()
            .filter_map(|(x, loc)| point::build(x, loc, remedies).ok())
            .collect()
    } else {
        Vec::new()
    };
    Review {
        outcome,
        points,
        matching,
        humanness,
        habits,
        katas,
        machine_katas,
        machine_gois,
        overused_katas,
        // どの一人称を選ぶかは、型にも語にも載らない。
        first_person: first_person_remedies(o.first_person, o.draft_first_person),
        // 書き出しの構造も、密度にも語の位置にも載らない。
        opening: opening_remedies(o.opening, o.draft_opening),
    }
}

/// 相手集合から離れている次元を、離れている順に言う。
///
/// 直し方は定義から取らない。 系統の次元は指標ではないので定義ファイルを
/// 持たない——観測がそのまま指示になる「本人より多い／少ない」を言う。
fn matching_remedies(observed: &[MatchingObserved]) -> Vec<String> {
    observed
        .iter()
        .take(MAX_POINTS)
        .map(|o| {
            let way = if o.theirs > o.mine {
                "少ない"
            } else {
                "多い"
            };
            // 「本人と同じ書き方で」を付ける。 数だけ言うと、数を満たす壊し方が
            // 選ばれる——文字種の空白を上げよと言われた側が、和文のあいだに空白を
            // 入れたことが実際に起きた（[0 段目](verdict::Stage::Writing)）。
            let fix = if o.theirs > o.mine {
                "本人と同じ書き方で増やす"
            } else {
                "減らす"
            };
            let mut line = format!(
                "{}の「{}」が本人より{way}（この文章 {:.3} / 本人 {:.3}）。{fix}と照合値が {:+.3} 動く。",
                o.system, o.dim, o.mine, o.theirs, o.effect
            );
            if !o.examples.is_empty() {
                // 本人がどう書いているかを見せる。 見せなければ、受け取った側は
                // その人の文章を自分で読みに行くことになる。
                line.push_str(&format!("本人はこう書いている: 「{}」。", o.examples.join("」「")));
            }
            if !o.spots.is_empty() {
                // この文章のどこかを見せる。 本人の実例だけでは、自分の文章の
                // どこを直すのかが分からない。
                line.push_str(&format!("この文章のここ: 「{}」。", o.spots.join("」「")));
            }
            line
        })
        .collect()
}

/// 機械の側に落ちている指標の直し方を、落ちている順に並べる。
///
/// 向きは観測が持っている。 較正から読んだもので、定義の名乗る向きとは限らない
/// ——同じ型で書かせた生成文は、先行研究の言う「機械の側」に来ないことがある。
fn humanness_remedies(
    observed: &[HumannessObserved],
    remedies: &dyn Remedies,
    ceiling_hit: &[&str],
) -> Vec<String> {
    // 本人へ寄せると合算が上がる指標だけを渡す。
    //
    // 0 と比べない。 0 は人と機械の境目であって、その人のところではない。
    // 従うと悪くなる直し方も渡さない——指標どうしが正反対を向くことがあり、
    // 渡せば受け取った側は自分の手で判定を悪くする。
    let mut low: Vec<&HumannessObserved> = observed.iter().filter(|o| o.effect > 0.0).collect();
    // 効く量の大きい順。 落ちている深さで並べると、正反対を指す 2 本の
    // どちらが勝つかを言えない——実測で、受け取った側が逆を選んだ。
    low.sort_by(|a, b| {
        b.effect
            .partial_cmp(&a.effect)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    low.iter()
        .filter_map(|o| {
            // 観測が持っている向きで選ぶ。 上げるなら「足りない側の直し方」、
            // 下げるなら「多すぎる側の直し方」——[指摘](point::build)が外れた向きで
            // 選ぶのと同じ規則である。
            //
            // 下と決め打ってはいけない。 人らしさの指標は両側の直し方を持つので、
            // 決め打つと必ず「足す」側が返り、較正が「減らせ」と言った場面でも
            // 「増やせ」と指示する——直し方に従うほど人らしさが下がる。
            let remedy = if o.raise {
                remedies.lower(&o.name).or_else(|| remedies.upper(&o.name))
            } else {
                remedies.upper(&o.name).or_else(|| remedies.lower(&o.name))
            }?;
            // 並べる値はその指標だけで見た人らしさであって、指標そのものの
            // 量ではない。 「密度が本人より足りない」と書けば、続く直し方の
            // 「減らす」と食い違って読める。
            let way = if o.target > o.value { "近い" } else { "遠い" };
            let mut line = format!(
                "{}で見た基準との距離が本人より{way}（この文章 {:.3} / 本人 {:.3}）。{remedy}基準との距離が {:+.3} 動く。",
                o.name, o.value, o.target, o.effect
            );
            // 本人が現に繰り返している言い回しを添える。 添えなければ、
            // 受け取った側は自分ででっち上げた定型句を挿し込む。
            //
            // 上限を超えているものは外す。 残せば同じ言い回しに増やせと減らせが
            // 同時に出て、どちらに従っても片方の指摘に背くことになる。
            let phrases: Vec<&str> = o
                .phrases
                .iter()
                .map(String::as_str)
                .filter(|p| !ceiling_hit.contains(p))
                .collect();
            if !phrases.is_empty() {
                line.push_str(&format!(
                    " 本人が繰り返しているのは「{}」。",
                    phrases.join("」「")
                ));
            }
            if !o.once_only.is_empty() {
                // どれが散らしているのかを言う。 言わなければ、受け取った側は
                // 言い換えを別の言い換えに置き換えることになる。
                line.push_str(&format!(
                    " 一度しか出てこない語: 「{}」。",
                    o.once_only.join("」「")
                ));
            }
            if !o.overused.is_empty() {
                // どれを減らすのかを言う。 言わなければ、受け取った側は
                // 手当たり次第に言い換えることになる。
                line.push_str(&format!(
                    " この文章が繰り返しているのは「{}」。",
                    o.overused.join("」「")
                ));
            }
            Some(line)
        })
        .collect()
}

/// 判定と指摘を決める閾値。指紋に入れる材料である。
///
/// 平文で返す。 ハッシュにすると、合わないときにどれが変わったかを言えない。
///
/// ここで返すのは、このクレートが持つ閾値だけである。 目盛りの側の閾値は
/// `kakiburi-scale` が自分で返す——ここから手を伸ばせば、
/// [依存してはいけない向き](../../../docs/design/000-architecture.md#kakiburi-review)の
/// 依存ができる。
#[must_use]
pub fn settings() -> Vec<(&'static str, String)> {
    vec![
        ("review::APPEARANCE_FLOOR", APPEARANCE_FLOOR.to_string()),
        ("review::OVERUSE", OVERUSE.to_string()),
        ("review::MAX_POINTS", MAX_POINTS.to_string()),
        ("review::OPENING_MIN", OPENING_MIN.to_string()),
        ("review::OPENING_SHARE", OPENING_SHARE.to_string()),
        ("review::FIRST_PERSON_MIN", FIRST_PERSON_MIN.to_string()),
        ("review::FIRST_PERSON_SHARE", FIRST_PERSON_SHARE.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    struct All;
    impl Remedies for All {
        fn upper(&self, name: &str) -> Option<String> {
            Some(format!("{name}を減らす。"))
        }
        fn lower(&self, name: &str) -> Option<String> {
            Some(format!("{name}を足す。"))
        }
    }

    struct None_;
    impl Remedies for None_ {
        fn upper(&self, _: &str) -> Option<String> {
            None
        }
        fn lower(&self, _: &str) -> Option<String> {
            None
        }
    }

    fn observed(name: &str, value: f64, low: f64, high: f64) -> Observed {
        Observed {
            name: name.into(),
            value: Some(value),
            range: Range {
                low,
                high,
                units: 10,
            },
            lower: Lower::Spread,
            direct: false,
        }
    }

    #[test]
    fn 通るときは指摘が無い() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.outcome.verdict, Verdict::Pass);
        assert!(r.points.is_empty());
        assert!(!r.has_aside(), "知らせが無ければ見出しも要らない");
    }

    #[test]
    fn 代わりの値で出した照合値は検めでも通らないにしない() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Machine),
                matching_substituted: Some("読点の打ち方"),
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert_eq!(r.outcome.stage, Stage::Matching);
    }

    #[test]
    fn 通っても幅の外の知らせは残る() {
        // 判定を止めない軸が幅の外にあるとき、「すべて幅の中にある」と
        // 「幅の外にある」が同じ画面に並ぶ。見せる側が分けられるように、
        // 知らせがあることを言えなければならない。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [observed("鉤括弧", 8.110, 0.0, 6.697)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &h,
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.outcome.verdict, Verdict::Pass);
        assert_eq!(r.outcome.reason, "前に出す指標がすべて幅の中にある");
        assert!(!r.habits.is_empty(), "幅の外なので知らせは出る");
        assert!(r.has_aside());
    }

    #[test]
    fn 本人と違う一人称を名指しする() {
        // 本人が `私` をまったく使わないわけではない。選んでいるのが `僕` である。
        let person = [("僕".to_owned(), 0.571), ("私".to_owned(), 0.102)];
        let draft = [("私".to_owned(), 3)];
        let got = first_person_remedies(&person, &draft);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("「私」をこの文章に 3 回"), "{got:?}");
        assert!(got[0].contains("本人は「僕」と書く"), "{got:?}");
    }

    #[test]
    fn 本人と同じ一人称なら何も言わない() {
        let person = [("僕".to_owned(), 0.571), ("私".to_owned(), 0.102)];
        assert!(first_person_remedies(&person, &[("僕".to_owned(), 5)]).is_empty());
    }

    #[test]
    fn 一人称が出てこない文章には言わない() {
        // 主語を置かない文章はふつうにある。 実測で本人自身が 49 本中 21 本で
        // どの一人称も使っていない。
        let person = [("僕".to_owned(), 0.571)];
        assert!(first_person_remedies(&person, &[]).is_empty());
        assert!(first_person_remedies(&person, &[("私".to_owned(), 0)]).is_empty());
    }

    #[test]
    fn 本人が一人称を選んでいなければ言わない() {
        // 1 本で使っただけのものを「本人はこう書く」と言えば、書き手は
        // 自分が一度しか書かなかった語に寄せることになる。
        let person = [("僕".to_owned(), 0.05)];
        assert!(first_person_remedies(&person, &[("私".to_owned(), 3)]).is_empty());
        assert!(first_person_remedies(&[], &[("私".to_owned(), 3)]).is_empty());
    }

    #[test]
    fn 本人と違う書き出しを名指しする() {
        // 見出しの密度では言えない。 1 番目にあっても 3 番目にあっても同じ値になる。
        let person = [("段落".to_owned(), 0.92), ("見出し".to_owned(), 0.06)];
        let got = opening_remedies(&person, Some("見出し"));
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("見出しで始まっている"), "{got:?}");
        assert!(got[0].contains("92% の記事を段落で"), "{got:?}");
    }

    #[test]
    fn 本人と同じ書き出しなら何も言わない() {
        let person = [("段落".to_owned(), 0.92), ("見出し".to_owned(), 0.06)];
        assert!(opening_remedies(&person, Some("段落")).is_empty());
    }

    #[test]
    fn 書き出しの無い文章には言わない() {
        let person = [("段落".to_owned(), 0.92)];
        assert!(opening_remedies(&person, None).is_empty());
    }

    #[test]
    fn 本人が始め方を選んでいなければ言わない() {
        // 半々で始めている書き手に「こう始めろ」とは言えない。
        let person = [("段落".to_owned(), 0.52), ("見出し".to_owned(), 0.48)];
        assert!(opening_remedies(&person, Some("見出し")).is_empty());
        assert!(opening_remedies(&[], Some("見出し")).is_empty());
    }

    fn human(name: &str, value: f64) -> HumannessObserved {
        HumannessObserved {
            name: name.into(),
            value,
            raise: true,
            phrases: Vec::new(),
            overused: Vec::new(),
            once_only: Vec::new(),
            effect: 0.1,
            target: 1.0,
        }
    }

    fn diverge(system: &str, dim: &str, mine: f64, theirs: f64) -> MatchingObserved {
        MatchingObserved {
            system: system.into(),
            dim: dim.into(),
            mine,
            theirs,
            effect: 0.05,
            examples: Vec::new(),
            spots: Vec::new(),
        }
    }

    #[test]
    fn 照合で止まったら離れている次元を渡す() {
        // ここで何も出さなければ、その人へ寄せるという目的そのものに対して
        // 道具が黙る。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let v = [diverge("機能語", "一方", 18.6, -0.2)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::InBand),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &v,
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.outcome.stage, Stage::Matching);
        assert!(r.points.is_empty(), "書きぶりの枠は奪わない");
        assert_eq!(r.matching.len(), 1, "{:?}", r.matching);
        assert!(r.matching[0].contains("一方"), "{:?}", r.matching);
        assert!(r.matching[0].contains("多い"), "{:?}", r.matching);
        assert!(r.matching[0].contains("減らす"), "{:?}", r.matching);
    }

    #[test]
    fn 本人のほうが多ければ増やすと言う() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let v = [diverge("機能語", "のだ", -1.2, 2.4)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::InBand),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &v,
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert!(r.matching[0].contains("少ない"), "{:?}", r.matching);
        assert!(r.matching[0].contains("増やす"), "{:?}", r.matching);
    }

    #[test]
    fn 照合の直し方も枠に上限がある() {
        // 数え上げても直せない。書きぶりと同じ上限を掛ける。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let v: Vec<MatchingObserved> = (0..10)
            .map(|i| diverge("機能語", &format!("語{i}"), f64::from(i), 0.0))
            .collect();
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::InBand),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &v,
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.matching.len(), MAX_POINTS, "{:?}", r.matching);
    }

    #[test]
    fn 照合を通ったら照合の直し方は出さない() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let v = [diverge("機能語", "一方", 18.6, -0.2)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &v,
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert!(r.matching.is_empty(), "{:?}", r.matching);
    }

    #[test]
    fn 人らしさで止まったら人らしさの直し方を渡す() {
        // **ここで何も出さなければ、機械の書いた草稿は止められるだけで直せない。**
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut ok = human("繰り返し", -1.2);
        ok.effect = 0.7;
        let mut done = human("圧縮率", 0.4);
        done.effect = 0.0;
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[ok, done],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.outcome.stage, Stage::Humanness);
        assert!(r.points.is_empty(), "書きぶりの枠は奪わない");
        assert_eq!(r.humanness.len(), 1, "効く量があるのは 1 本");
        assert!(r.humanness[0].contains("繰り返し"), "{:?}", r.humanness);
        assert!(r.humanness[0].contains("足す"), "{:?}", r.humanness);
    }

    #[test]
    fn 閾値は名前と値を平文で返す() {
        // どれが変わったかを指紋の差として名指せるようにする。
        let s = settings();
        assert!(
            s.contains(&("review::APPEARANCE_FLOOR", APPEARANCE_FLOOR.to_string())),
            "{s:?}"
        );
        assert!(
            s.contains(&("review::OVERUSE", OVERUSE.to_string())),
            "{s:?}"
        );
        let mut names: Vec<&str> = s.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), s.len(), "名前が重なれば片方が消える");
    }

    #[test]
    fn 人らしさの直し方は並べた値が人らしさだと言う() {
        // 並べる値は指標ごとの人らしさであって、指標そのものの密度ではない。
        // 「句読点の密度が本人より足りない」と書くと、続く「読点を減らす」と
        // 食い違って読める。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut down = human("句読点の密度", -0.9);
        down.raise = false;
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[down],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.humanness.len(), 1, "{:?}", r.humanness);
        let line = &r.humanness[0];
        assert!(
            line.starts_with(
                "句読点の密度で見た基準との距離が本人より近い（この文章 -0.900 / 本人 1.000）"
            ),
            "{line}"
        );
        assert!(
            line.contains("句読点の密度を減らす。"),
            "直し方は残す: {line}"
        );
        assert!(
            line.contains("基準との距離が +0.100 動く"),
            "動く量も残す: {line}"
        );
    }

    #[test]
    fn 減らす側では繰り返しすぎているものを名指す() {
        // 「減らせ」と言うなら、どれを減らすのかを言わなければ直せない。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut h = human("短い繰り返し", -0.2);
        h.raise = false;
        h.overused = vec!["ます。".into(), "ています".into()];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[h],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert!(r.humanness[0].contains("ます。"), "{:?}", r.humanness);
    }

    #[test]
    fn 言い回しを持つなら添える() {
        // 数値と向きだけでは直せない。 受け取った側が自分ででっち上げた
        // 定型句を挿し込むことになる。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut h = human("長い繰り返し", -0.4);
        h.phrases = vec!["と思っています".into(), "しています。".into()];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[h],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert!(
            r.humanness[0].contains("と思っています"),
            "{:?}",
            r.humanness
        );
        assert!(r.humanness[0].contains("しています。"), "{:?}", r.humanness);
    }

    #[test]
    fn 使いすぎと言った言い回しを繰り返せとは言わない() {
        // 同じ言い回しに、増やせと減らせが同時に出ていた。 実測で当たった——
        // 「しています。」は本人が繰り返している言い回しなので繰り返せの一覧に
        // 入り、同時に本人の上限を超えていたので減らせにも出た。
        //
        // 受け取った側は、どちらに従っても片方の指摘に背く。減らした結果、
        // 人らしさ値は 0.788 から 0.484 へ下がった。上限が勝つ。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut h = human("長い繰り返し", -0.4);
        h.phrases = vec!["ています。".into(), "しています。".into()];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[h],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[dense("しています。", 10, 4.4, 2.8)],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert!(
            r.overused_katas.iter().any(|s| s.contains("しています。")),
            "上限を超えているのに減らせと言っていない: {:?}",
            r.overused_katas
        );
        assert!(
            !r.humanness.iter().any(|s| s.contains("しています。")),
            "減らせと言った言い回しを、繰り返せの一覧にも渡している: {:?}",
            r.humanness
        );
        assert!(
            r.humanness.iter().any(|s| s.contains("ています。")),
            "上限に触れていない言い回しまで落としている: {:?}",
            r.humanness
        );
    }

    #[test]
    fn 従うと悪くなる直し方は渡さない() {
        // 指標どうしが正反対を向くことがある。 渡せば、受け取った側は
        // 自分の手で判定を悪くする。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut bad = human("語彙の豊富さ", -0.06);
        bad.effect = -0.95;
        let mut good = human("圧縮率", -0.78);
        good.effect = 0.99;
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[bad, good],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.humanness.len(), 1, "{:?}", r.humanness);
        assert!(r.humanness[0].contains("圧縮率"), "{:?}", r.humanness);
    }

    #[test]
    fn 人らしさの直し方は効く量の順に並ぶ() {
        // 落ちている深さで並べてはいけない。 正反対を指す 2 本のどちらが
        // 勝つかを言えず、受け取った側が逆を選ぶ。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut deep = human("繰り返し", -1.5);
        deep.effect = 0.05;
        let mut shallow = human("圧縮率", -0.3);
        shallow.effect = 0.90;
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[deep, shallow],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert!(
            r.humanness[0].contains("圧縮率"),
            "深いほうではない: {:?}",
            r.humanness
        );
        assert!(r.humanness[1].contains("繰り返し"), "{:?}", r.humanness);
    }

    #[test]
    fn 本人に届いている指標は直させない() {
        // 直しても合算は上がらない。効く量が 0 以下なら渡さない。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut a = human("繰り返し", 0.8);
        a.effect = 0.0;
        let mut b = human("圧縮率", 0.4);
        b.effect = -0.2;
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[a, b],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert!(r.humanness.is_empty(), "{:?}", r.humanness);
    }

    #[test]
    fn 帯の中で止まっても人らしさの直し方を渡す() {
        // 帯の中から人の側へ出る道を示さなければ、受け取った側は動けない。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("繰り返し", -1.2)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::InBand),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &h,
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.outcome.stage, Stage::Humanness);
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert_eq!(r.humanness.len(), 1, "{:?}", r.humanness);
    }

    #[test]
    fn 人らしさが測れないときは直し方を出さない() {
        // **測れていないことと、機械の側にあることは違う。** 混ぜれば、短いだけの
        // 文章に「繰り返しを足せ」と言うことになる。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: None,
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert!(r.humanness.is_empty());
    }

    #[test]
    fn 通るときは人らしさの直し方も無い() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("繰り返し", -1.2)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &h,
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert!(r.humanness.is_empty(), "{:?}", r.humanness);
    }

    #[test]
    fn 寄せる向きは観測が持つ() {
        // 人らしさの指標は両側の直し方を持つ。 下と決め打てば必ず「足す」側が
        // 返り、較正が「減らせ」と言った場面でも「増やせ」と指示する——直し方に
        // 従うほど人らしさが下がる。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];

        let mut down = human("圧縮率", -0.9);
        down.raise = false;
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[down],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.humanness.len(), 1, "{:?}", r.humanness);
        assert!(r.humanness[0].contains("減らす"), "{:?}", r.humanness);

        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[human("圧縮率", -0.9)],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert!(r.humanness[0].contains("足す"), "{:?}", r.humanness);
    }

    #[test]
    fn 片側しか無い指標でも直し方を出す() {
        // 向きが「上げる」でも、定義が下側の文面を持たないなら上側で出す。
        // 3 つ揃わないものを出さないより、持っている側で出すほうが直せる。
        struct UpperOnly;
        impl Remedies for UpperOnly {
            fn upper(&self, name: &str) -> Option<String> {
                Some(format!("{name}を減らす。"))
            }
            fn lower(&self, _: &str) -> Option<String> {
                None
            }
        }
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("圧縮率", -0.9)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &h,
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &UpperOnly,
        );
        assert_eq!(r.humanness.len(), 1, "{:?}", r.humanness);
        assert!(r.humanness[0].contains("減らす"), "{:?}", r.humanness);
    }

    #[test]
    fn 直し方が定義に無ければ出さない() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("繰り返し", -1.2)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &h,
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &None_,
        );
        assert!(r.humanness.is_empty(), "{:?}", r.humanness);
    }

    #[test]
    fn 前の段で止まったら指摘を出さない() {
        // 出せば、受け取った側は照合値を上げようとして人らしさを下げる。
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.outcome.stage, Stage::Humanness);
        assert!(r.points.is_empty(), "指摘を出してはいけない");
    }

    #[test]
    fn 照合値で止まっても指摘を出さない() {
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::InBand),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.outcome.stage, Stage::Matching);
        assert!(r.points.is_empty());
    }

    #[test]
    fn 三段目まで来たら指摘を組む() {
        let d = [
            observed("全角括弧", 5.0, 0.0, 2.0),
            observed("感嘆符", 10.0, 0.0, 1.0),
        ];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert_eq!(r.points.len(), 2);
        // 外れの大きさの降順。感嘆符は 9.0、全角括弧は 1.5。
        assert_eq!(r.points[0].name, "感嘆符");
    }

    #[test]
    fn 指摘は判定と同じ集合から取る() {
        // 止めた理由が指摘に出てこないという食い違いを防ぐ。
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert!(
            r.outcome.reason.contains("全角括弧"),
            "{}",
            r.outcome.reason
        );
        assert_eq!(r.points[0].name, "全角括弧");
    }

    #[test]
    fn 直し方の無い指標は指摘に出ない() {
        // だが判定は止まったままである——言えないもので止めない、の逆側は
        // 「言えるものがある以上、言って次の周へ回す」である。
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &None_,
        );
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert!(r.points.is_empty());
    }

    #[test]
    fn 指摘は_4_本までである() {
        let d: Vec<Observed> = (0..10)
            .map(|i| observed(&format!("m{i:02}"), 5.0, 0.0, 2.0))
            .collect();
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                matching_substituted: None,
                too_short: None,
                directives: &d,
                directives_muted: false,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
                machine_katas: &[],
                machine_gois: &[],
                phrases: &[],
                first_person: &[],
                draft_first_person: &[],
                opening: &[],
                draft_opening: None,
            },
            &All,
        );
        assert_eq!(r.points.len(), MAX_POINTS);
    }

    fn dense(text: &str, times: usize, density: f64, ceiling: f64) -> Kata {
        Kata {
            base: 0.0,
            text: text.into(),
            rate: 0.5,
            at: 0.5,
            used: true,
            spots: Vec::new(),
            times,
            density,
            ceiling,
        }
    }

    #[test]
    fn 本人の上限を大きく超えたら使いすぎと言う() {
        let over = overused_katas(&[dense("ことができます", 10, 1.0, 0.3)]);
        assert_eq!(over.len(), 1, "{over:?}");
        assert!(over[0].contains("ことができます"), "{over:?}");
        assert!(over[0].contains("10 回"), "{over:?}");
    }

    #[test]
    fn 上限のちょうどでは言わない() {
        // 幅の外と同じ扱いである。 超えたときだけ言う。
        assert!(overused_katas(&[dense("ています", 3, 0.3, 0.3)]).is_empty());
    }

    #[test]
    fn 本人も使う語を使わないと言い切らない() {
        // 選ぶ条件は本人側の割合に上限を置いているだけで、0 を求めていない。
        // 言い切れば、自分の文章に現に在る語を無いと言われる。
        let goi = |base: f64| Goi {
            text: "地味".to_owned(),
            rate: 0.33,
            base,
            times: 7,
            theirs: vec!["簡単".to_owned()],
        };
        let never = machine_goi_remedies(&[goi(0.0)]);
        assert!(never[0].contains("本人は使わない"), "{never:?}");
        let sometimes = machine_goi_remedies(&[goi(0.05)]);
        assert!(
            sometimes[0].contains("本人は 5% でしか使わない"),
            "{sometimes:?}"
        );
        assert!(
            !sometimes[0].contains("本人は使わない"),
            "使うのに使わないと言っている: {sometimes:?}"
        );
    }

    #[test]
    fn 本人が使わない並びは使いすぎと言わない() {
        // 上限が 0 のものは、そもそも本人の型ではない。
        assert!(overused_katas(&[dense("と考えています", 9, 2.0, 0.0)]).is_empty());
    }

    fn kata(text: &str, rate: f64, at: f64, used: bool) -> Kata {
        Kata {
            base: 0.0,
            text: text.into(),
            rate,
            at,
            used,
            spots: Vec::new(),
            times: usize::from(used),
            density: 0.0,
            ceiling: 0.0,
        }
    }

    #[test]
    fn 使われている型は渡さない() {
        let got = kata_remedies(&[kata("となります。", 0.3, 0.5, true)]);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn 使われていない型は割合とともに渡す() {
        let got = kata_remedies(&[kata("となります。", 0.3, 0.5, false)]);
        assert_eq!(got.len(), 1);
        assert!(got[0].contains("となります。"), "{}", got[0]);
        assert!(got[0].contains("30%"), "{}", got[0]);
    }

    #[test]
    fn 位置の偏った型はどこで使うかまで言う() {
        // **そこで使うから型なのであって、どこかに混ぜればよいものではない。**
        let head = kata_remedies(&[kata("ご無沙汰しております", 0.26, 0.03, false)]);
        assert!(head[0].contains("書き出しで"), "{}", head[0]);
        let tail = kata_remedies(&[kata("以上です。", 0.26, 0.95, false)]);
        assert!(tail[0].contains("結びで"), "{}", tail[0]);
        let mid = kata_remedies(&[kata("となります。", 0.3, 0.5, false)]);
        assert!(!mid[0].contains("書き出し"), "{}", mid[0]);
    }

    #[test]
    fn 型は広く使う順に渡す() {
        let got = kata_remedies(&[
            kata("たまに書く", 0.26, 0.5, false),
            kata("よく書く", 0.42, 0.5, false),
        ]);
        assert!(got[0].contains("よく書く"), "{}", got[0]);
    }
}

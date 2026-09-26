//! 終了コード。
//!
//! 判定できないを 1 と分けるのが要点である。 一緒にすれば、分からないと言えることが
//! 呼ぶ側から消える。

use kuchiyose_review::Verdict;

/// 終了コード。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// 通る。
    Pass,
    /// 通らない。
    Fail,
    /// 判定できない。
    ///
    /// 目盛りが組み立てられなかったときもここである。 素材が足りない、長さの範囲が
    /// 重ならない、帯が重なりすぎる、自己検査が崩れた——どれも正常な状態であり、
    /// 仕様がそのために判定できないを置いている。
    Unknown,
    /// 使い方の誤り。
    Usage,
    /// 形代が読めない。
    Unreadable,
    /// 指紋が環境と合わない。
    FingerprintMismatch,
    /// 環境の側の理由で測れない。道具が無い、道具が失敗した。
    ///
    /// 素材を足しても直らない。 直すのは環境である。
    Environment,
}

impl Exit {
    /// 数値。
    ///
    /// `64` 以上は使う前の問題に取っておく——形代が壊れている、読めない、
    /// 指紋が合わない。判定できないを返す経路をエラーとして扱わない。
    #[must_use]
    pub fn code(self) -> i32 {
        match self {
            Exit::Pass => 0,
            Exit::Fail => 1,
            Exit::Unknown => 2,
            Exit::Usage => 64,
            Exit::Unreadable => 65,
            Exit::FingerprintMismatch => 66,
            Exit::Environment => 69,
        }
    }

    /// 判定の 3 値から。
    #[must_use]
    pub fn from_verdict(v: Verdict) -> Self {
        match v {
            Verdict::Pass => Exit::Pass,
            Verdict::Fail => Exit::Fail,
            Verdict::Unknown => Exit::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 判定できないは通らないと分かれる() {
        assert_ne!(Exit::Unknown.code(), Exit::Fail.code());
        assert_eq!(Exit::Unknown.code(), 2);
        assert_eq!(Exit::Fail.code(), 1);
    }

    #[test]
    fn 使う前の問題は_64_以上である() {
        for e in [
            Exit::Usage,
            Exit::Unreadable,
            Exit::FingerprintMismatch,
            Exit::Environment,
        ] {
            assert!(e.code() >= 64, "{e:?} が {} になっている", e.code());
        }
    }

    #[test]
    fn 判定の_3_値はすべて_64_未満である() {
        // 判定できないを返す経路をエラーとして扱わない。
        for v in [Verdict::Pass, Verdict::Fail, Verdict::Unknown] {
            assert!(Exit::from_verdict(v).code() < 64);
        }
    }

    #[test]
    fn 目盛りが組み立てられなければ_64_ではなく_2_である() {
        // 素材が足りずに組み立てられなかったのは正常な状態である。
        assert_eq!(Exit::Unknown.code(), 2);
    }

    #[test]
    fn 判定の_3_値と終了コードが_1_対_1_である() {
        assert_eq!(Exit::from_verdict(Verdict::Pass), Exit::Pass);
        assert_eq!(Exit::from_verdict(Verdict::Fail), Exit::Fail);
        assert_eq!(Exit::from_verdict(Verdict::Unknown), Exit::Unknown);
    }
}

#!/usr/bin/env perl
# 較正を入れた版。仕様どおりの経路を端から端まで通す。
#
#   1  系統ごとに 2 本の距離を出す（Cosine Delta）
#   2  系統ごとに、同じ人の対と違う人の対の距離分布に照らして尤度比に変える
#      （ロジスティック回帰。正則化あり）
#   3  尤度比を合算して 1 つの照合値にする（これもロジスティック回帰）
#   4  1 本 × 相手集合 の中央値を、その 1 本の点とする
#   5  天井と床の分布から帯を読む
#
# 照合値は大きいほど同じ人らしい。だから天井が上、床が下になる。
use strict; use warnings;
use utf8; use open qw(:std :utf8);
use List::Util qw(sum min max shuffle);
use Encode qw(decode_utf8);

my ($pg, $bg, @opt) = @ARGV;
die "usage: spike-calibrate.pl <本人 glob> <基準 glob> [--drop 系統] [--rotate N]\n"
  unless $pg && $bg;
my (%drop, $rotate, $types_only, $types_ja, $drop_nonja_cells, @check, $commas_only);
while (@opt) {
  my $k = shift @opt;
  if    ($k eq '--drop')   { my $v = shift @opt; $drop{decode_utf8($v)} = 1 if defined $v }
  elsif ($k eq '--rotate') { $rotate = shift @opt }
  elsif ($k eq '--types')  { $types_only = 1 }
  elsif ($k eq '--types-ja') { $types_ja = 1 }
  elsif ($k eq q{--drop-nonja-cells}) { $drop_nonja_cells = 1 }
  elsif ($k eq q{--check}) { push @check, shift @opt }
  elsif ($k eq q{--commas}) { $commas_only = 1 }
}

# ---- 日本語の文字 ----------------------------------------------------
# スクリプト属性で書いてはいけない。\p{Hiragana} は Script_Extensions で
# 照合されるため 。 、 まで飲み、約物が分母に入る。
#
# ブロックでも書いてはいけない。かなのブロックの中に約物がある——
# ・ (U+30FB)、゠ (U+30A0)、濁点 (U+3099〜U+309C)。・ を入れると中黒が
# 分母に入り、中黒の多い書き手ほど中黒の率が下がる。
our $HIRA  = qr/[\x{3041}-\x{3096}\x{309D}-\x{309F}]/;
# ー 々 はカタカナに入れる（指標「文字種」の分類に合わせる）
our $KATA  = qr/[\x{30A1}-\x{30FA}\x{30FC}-\x{30FF}\x{FF66}-\x{FF9F}\x{3005}]/;
our $KANJI = qr/[\x{2F00}-\x{2FDF}\x{3400}-\x{4DBF}\x{4E00}-\x{9FFF}\x{F900}-\x{FAFF}\x{20000}-\x{2FA1F}]/;
our $JA    = qr/$HIRA|$KATA|$KANJI/;
# 約物は閉じた列挙である。広く取ると「その他」が受け皿でなくなる。
our $YAKU  = qr/[。、！？!?「」『』（）()・…〜～]/;
# セルが地の文かを見るときは繰り返し記号と長音符を数に入れない。単独で語に
# ならず、ー は比較表の「該当なし」に使われる。
our $JA_PROSE = qr/[\x{3041}-\x{3096}\x{309F}\x{30A1}-\x{30FA}\x{30FF}\x{FF66}-\x{FF6F}\x{FF71}-\x{FF9F}]|$KANJI/;

# ---- 取り込み --------------------------------------------------------
sub prose {
  my $p = shift;
  open my $h, '<:utf8', $p or die "$p: $!";
  my (@o, $fence, $front);
  while (my $l = <$h>) {
    chomp $l;
    if ($. == 1 && $l =~ /^---\s*$/) { $front = 1; next }
    if ($front) { $front = 0 if $l =~ /^---\s*$/; next }
    if ($l =~ /^\s*```/) { $fence = !$fence; next }
    next if $fence || $l =~ /^\s*$/ || $l =~ /^\s*\|?\s*-{3,}/;
    $l =~ s/`[^`]*`//g; $l =~ s/<[^>]+>//g;
    $l =~ s/\[([^\]]*)\]\([^)]*\)/$1/g;
    $l =~ s/^\s*[>#*\-]+\s*//; $l =~ s/^\s*\d+\.\s*//;
    # 表のセルは 1 つずつ別の node である。行のままにすると桁揃えの空白が
    # 地の文に入り、書きぶりではなく整形を測る。
    if ($l =~ /\|/) {
      for my $cell (split /\|/, $l) {
        $cell =~ s/^\s+//; $cell =~ s/\s+$//;
        next if $cell =~ /^\s*$/;
        # `o` `-` だけのセルは日本語の散文ではない。コードを外すのと同じ理由。
        next if $drop_nonja_cells && $cell !~ $JA_PROSE;
        push @o, $cell;
      }
      next;
    }
    push @o, $l unless $l =~ /^\s*$/;
  }
  close $h; return \@o;
}

sub features {
  my $lines = shift;
  my (%bi, %bf, %af, %ty, $ja);
  for my $l (@$lines) {
    my @c = split //, $l;
    for my $i (0 .. $#c) {
      my $c = $c[$i];
      $bi{"$c[$i]$c[$i+1]"}++ if $i < $#c;
      if ($c eq '、') {
        $bf{$i > 0 ? $c[$i-1] : '^'}++;
        $af{$i < $#c ? $c[$i+1] : '$'}++;
      }
      # 仕様の 10 分類。最後の「その他」が真の受け皿になるよう、
      # 上の 9 つはすべて閉じた範囲で書く。
      $ty{ $c =~ $HIRA  ? 'hira'
         : $c =~ $KATA  ? 'kata'
         : $c =~ $KANJI ? 'kanji'
         : $c =~ /[A-Za-z]/               ? 'alpha_h'
         : $c =~ /[\x{FF21}-\x{FF3A}\x{FF41}-\x{FF5A}]/ ? 'alpha_f'
         : $c =~ /[0-9]/                  ? 'digit_h'
         : $c =~ /[\x{FF10}-\x{FF19}]/    ? 'digit_f'
         : $c =~ $YAKU                    ? 'yaku'
         : $c =~ /[ \t\x{3000}]/          ? 'space'
         : 'other' }++;
      $ja++ if $c =~ $JA;
    }
  }
  # 日本語 3 種だけに絞った文字種。ラテン文字・記号の量は題材が強制するので、
  # 「書き手の選択だけで分離するか」を切り分けるために別に持つ。
  my %tyja = map { $_ => $ty{$_} || 0 } qw(hira kata kanji);
  return { bi => \%bi, before => \%bf, after => \%af,
           types => \%ty, types_ja => \%tyja, ja => $ja || 0 };
}

sub load {
  my @u;
  for my $p (sort glob shift) {
    my $f = features(prose($p));
    (my $n = $p) =~ s{.*/}{};
    # 日本語以外が主なら断る。分母から約物と空白を外す。
    my $t = $f->{types};
    my $den = sum(map { $t->{$_} || 0 } qw(hira kanji kata alpha_h alpha_f digit_h digit_f other)) || 1;
    my $ja  = sum(map { $t->{$_} || 0 } qw(hira kanji kata)) || 0;
    if ($ja / $den < 0.3) {
      printf STDERR "断る %s（日本語 %.0f%%）\n", $n, 100 * $ja / $den;
      next;
    }
    next if $f->{ja} < 1000;
    push @u, { name => $n, f => $f };
  }
  return @u;
}

my @person = load($pg);
my @base   = load($bg);
# 見る前に標本の長さの範囲を確かめる。揃っていなければ、離れていても
# 重なっても信じられない——長さと連動する指標がすべて離れて見える。
sub length_range_ok {
  my ($p, $b) = @_;
  my ($plo, $phi) = (min(map { $_->{f}{ja} } @$p), max(map { $_->{f}{ja} } @$p));
  my ($blo, $bhi) = (min(map { $_->{f}{ja} } @$b), max(map { $_->{f}{ja} } @$b));
  my $ov = min($phi, $bhi) - max($plo, $blo);
  $ov = 0 if $ov < 0;
  my ($pr, $br) = ($phi - $plo || 1, $bhi - $blo || 1);
  printf "長さ  本人 %d〜%d  基準 %d〜%d  重なり %d（本人の %.0f%%、基準の %.0f%%）\n",
         $plo, $phi, $blo, $bhi, $ov, 100 * $ov / $pr, 100 * $ov / $br;
  return $ov / $pr >= 0.5 && $ov / $br >= 0.5;
}

die "本人が 6 単位に届かない\n" if @person < 6;
die "基準が 4 単位に届かない\n" if @base < 4;

# 文字種の内訳を見る。分類の定義を書き写さずに済ませるため、ここに置く。
if ($types_only) {
  my @k = qw(hira kanji kata alpha_h alpha_f digit_h digit_f yaku space other);
  printf "%-34s %6s %s\n", '単位', '日本語', join ' ', map { sprintf '%6s', $_ } @k;
  # 長さと連動していないかを見る。連動していれば、離れて見えるのは長さの差である。
  sub pearson {
    my ($x, $y) = @_;
    my $n = @$x; return 0 if $n < 3;
    my ($mx, $my) = (sum(@$x) / $n, sum(@$y) / $n);
    my $c  = sum(map { ($x->[$_] - $mx) * ($y->[$_] - $my) } 0 .. $n - 1);
    my $sx = sqrt(sum(map { ($_ - $mx) ** 2 } @$x));
    my $sy = sqrt(sum(map { ($_ - $my) ** 2 } @$y));
    return ($sx && $sy) ? $c / ($sx * $sy) : 0;
  }
  for my $set (['本人', \@person], ['基準', \@base]) {
    printf "%s\n", '-' x 101;
    my (@len, %col);
    for my $u (@{$set->[1]}) {
      my $t = $u->{f}{types};
      my $tot = sum(values %$t) || 1;
      printf "%-34s %6d %s\n", substr($u->{name}, 0, 33), $u->{f}{ja},
             join ' ', map { sprintf '%5.1f%%', 100 * ($t->{$_} || 0) / $tot } @k;
      push @len, $u->{f}{ja};
      push @{$col{$_}}, 100 * ($t->{$_} || 0) / $tot for @k;
    }
    printf "%-34s %6s %s\n", "  $set->[0]：長さとの相関", '',
           join ' ', map { sprintf '%+6.2f', pearson(\@len, $col{$_}) } @k;
  }
  exit 0;
}

# 読点の打ち方を見る。指摘を当て推量でなく実測から作るために置いた。
if ($commas_only) {
  for my $set (['本人', \@person], ['基準', \@base]) {
    my (%bf, %af, $ja, $cm);
    for my $u (@{$set->[1]}) {
      $ja += $u->{f}{ja};
      $bf{$_} += $u->{f}{before}{$_} for keys %{$u->{f}{before}};
      $af{$_} += $u->{f}{after}{$_}  for keys %{$u->{f}{after}};
    }
    $cm = sum(values %bf) || 0;
    printf "%s  読点 %d 個 / 日本語 %d 字 = 1,000 字あたり %.1f\n",
           $set->[0], $cm, $ja, $ja ? 1000 * $cm / $ja : 0;
    for my $pair (['直前', \%bf], ['直後', \%af]) {
      my @k = sort { $pair->[1]{$b} <=> $pair->[1]{$a} || $a cmp $b } keys %{$pair->[1]};
      printf "  %s  %s\n", $pair->[0],
             join ' ', map { sprintf '%s(%.0f%%)', $_, 100 * $pair->[1]{$_} / ($cm || 1) }
                       @k[0 .. min(7, $#k)];
    }
  }
  exit 0;
}

my @systems = grep { !$drop{$_->{name}} } (
  { name => '文字bigram', key => 'bi',     n => 500 },
  { name => '読点前',      key => 'before', n => 50  },
  { name => '読点後',      key => 'after',  n => 50  },
  { name => '文字種',      key => ($types_ja ? 'types_ja' : 'types'), n => 10 },
);

# ---- 語彙と z 得点は割る前に 1 度だけ --------------------------------
my @all = (@person, @base);
my (%vec, %frozen);
for my $s (@systems) {
  my %tot;
  for my $u (@all) { $tot{$_} += $u->{f}{$s->{key}}{$_} for keys %{$u->{f}{$s->{key}}} }
  my @v = sort { $tot{$b} <=> $tot{$a} || $a cmp $b } keys %tot;
  my $vocab = [ @v[0 .. min($s->{n}, scalar @v) - 1] ];
  my @raw = map {
    my $u = $_; my $t = sum(values %{$u->{f}{$s->{key}}}) || 1;
    [ map { ($u->{f}{$s->{key}}{$_} // 0) / $t } @$vocab ];
  } @all;
  my $d = scalar @{$raw[0]};
  my (@mu, @sd);
  for my $j (0 .. $d - 1) {
    my @col = map { $_->[$j] } @raw;
    my $m = sum(@col) / @col;
    push @mu, $m; push @sd, sqrt(sum(map { ($_-$m)**2 } @col) / @col);
  }
  for my $i (0 .. $#all) {
    $vec{$s->{name}}{$all[$i]{name}} =
      [ map { $sd[$_] > 0 ? ($raw[$i][$_] - $mu[$_]) / $sd[$_] : 0 } 0 .. $d - 1 ];
  }
  # 検める文は、この語彙と z 得点に投影する。作り直さない。
  $frozen{$s->{name}} = { vocab => $vocab, mu => \@mu, sd => \@sd, key => $s->{key} };
}

# 固定した語彙・z 得点へ投影する。
sub project {
  my ($u) = @_;
  for my $s (@systems) {
    my $f = $frozen{$s->{name}};
    my $t = sum(values %{$u->{f}{$f->{key}}}) || 1;
    my @r = map { ($u->{f}{$f->{key}}{$_} // 0) / $t } @{$f->{vocab}};
    $vec{$s->{name}}{$u->{name}} =
      [ map { $f->{sd}[$_] > 0 ? ($r[$_] - $f->{mu}[$_]) / $f->{sd}[$_] : 0 } 0 .. $#r ];
  }
}

sub cosd {
  my ($a, $b) = @_;
  my ($dot, $na, $nb) = (0,0,0);
  for my $i (0 .. $#$a) { $dot += $a->[$i]*$b->[$i]; $na += $a->[$i]**2; $nb += $b->[$i]**2 }
  return 1 if $na == 0 || $nb == 0;
  return 1 - $dot/(sqrt($na)*sqrt($nb));
}
sub median { my @s = sort { $a <=> $b } @_; @s % 2 ? $s[$#s/2] : ($s[@s/2-1]+$s[@s/2])/2 }

# ---- ロジスティック回帰（L2 つき勾配降下）---------------------------
# X は行ごとの特徴、y は 1（同じ人）/ 0（違う人）。切片は自前で足す。
sub fit_logistic {
  my ($X, $y, $lambda) = @_;
  my $p = scalar @{$X->[0]};
  my @w = (0) x ($p + 1);
  my $lr = 0.5;
  for my $it (1 .. 4000) {
    my @g = (0) x ($p + 1);
    for my $i (0 .. $#$X) {
      my $z = $w[0];
      $z += $w[$_+1] * $X->[$i][$_] for 0 .. $p-1;
      my $s = 1 / (1 + exp(-max(-30, min(30, $z))));
      my $e = $s - $y->[$i];
      $g[0] += $e;
      $g[$_+1] += $e * $X->[$i][$_] for 0 .. $p-1;
    }
    my $n = scalar @$X;
    $w[0] -= $lr * $g[0] / $n;
    for my $j (1 .. $p) {
      $w[$j] -= $lr * ($g[$j] / $n + $lambda * $w[$j]);
    }
  }
  return \@w;
}
sub apply_logistic {
  my ($w, $x) = @_;
  my $z = $w->[0];
  $z += $w->[$_+1] * $x->[$_] for 0 .. $#$x;
  return $z;                     # 対数尤度比。大きいほど同じ人
}

# ---- 割る ------------------------------------------------------------
my @order = 0 .. $#person;
if (defined $rotate) {
  my $r = $rotate % scalar @person;
  @order = (@order[$r .. $#order], @order[0 .. $r-1]) if $r;
}
my @partners = map { $person[$_] } @order[0 .. 4];
my @ceilpts  = map { $person[$_] } @order[5 .. $#order];
my $bsplit   = int(@base / 2);
my @bcal     = @base[0 .. $bsplit-1];
my @bfloor   = @base[$bsplit .. $#base];

printf "本人 %d（相手集合 5 / 天井の点 %d）  基準 %d（較正 %d / 床の点 %d）\n",
       scalar @person, scalar @ceilpts, scalar @base, scalar @bcal, scalar @bfloor;
unless (length_range_ok(\@person, \@base)) {
  print "\n<< 長さの範囲が半分も重ならない。目盛りを作らない >>\n";
  exit 1;
}
printf "系統 %s%s\n", join('・', map { $_->{name} } @systems),
       defined $rotate ? "  回転 $rotate" : '';

# ---- 較正 ------------------------------------------------------------
# 同じ人の対 = 相手集合の中。違う人の対 = 基準の較正分 × 相手集合。
my (@cal_same, @cal_diff);
for my $i (0 .. $#partners) {
  for my $j ($i+1 .. $#partners) { push @cal_same, [$partners[$i], $partners[$j]] }
}
for my $b (@bcal) { push @cal_diff, [$b, $_] for @partners }

my %sysw;
for my $s (@systems) {
  my (@X, @y);
  for my $pr (@cal_same) { push @X, [ cosd($vec{$s->{name}}{$pr->[0]{name}}, $vec{$s->{name}}{$pr->[1]{name}}) ]; push @y, 1 }
  for my $pr (@cal_diff) { push @X, [ cosd($vec{$s->{name}}{$pr->[0]{name}}, $vec{$s->{name}}{$pr->[1]{name}}) ]; push @y, 0 }
  $sysw{$s->{name}} = fit_logistic(\@X, \@y, 0.1);
}

# 合算——系統ごとの対数尤度比を入力にして、もう一度当てはめる。
my (@FX, @Fy);
for my $pr (@cal_same, @cal_diff) {
  push @FX, [ map { apply_logistic($sysw{$_->{name}},
                    [ cosd($vec{$_->{name}}{$pr->[0]{name}}, $vec{$_->{name}}{$pr->[1]{name}}) ]) } @systems ];
}
push @Fy, (1) x scalar @cal_same;
push @Fy, (0) x scalar @cal_diff;
my $fw = fit_logistic(\@FX, \@Fy, 0.1);

printf "\n重み  切片 %+.3f", $fw->[0];
printf "  %s %+.3f", $systems[$_]{name}, $fw->[$_+1] for 0 .. $#systems;
print "\n";

# ---- 照合値 ----------------------------------------------------------
sub matching_value {
  my $u = shift;
  my @per;
  for my $p (@partners) {
    next if $p->{name} eq $u->{name};
    my @llr = map { apply_logistic($sysw{$_->{name}},
                    [ cosd($vec{$_->{name}}{$u->{name}}, $vec{$_->{name}}{$p->{name}}) ]) } @systems;
    push @per, apply_logistic($fw, \@llr);
  }
  return median(@per);
}

my @ceil  = map { matching_value($_) } @ceilpts;
my @floor = map { matching_value($_) } @bfloor;

printf "\n%-8s %-9s %-9s %-9s %s\n", '', '下端', '上端', '中央', '広がり';
printf "%s\n", '-' x 50;
printf "%-8s %-9.3f %-9.3f %-9.3f %.3f\n", '天井', min(@ceil),  max(@ceil),  median(@ceil),  max(@ceil)-min(@ceil);
printf "%-8s %-9.3f %-9.3f %-9.3f %.3f\n", '床',   min(@floor), max(@floor), median(@floor), max(@floor)-min(@floor);

# 帯——照合値は大きいほど本人側。天井の下端と床の上端を見る。
my ($clo, $chi) = (min(@ceil),  max(@ceil));
my ($flo, $fhi) = (min(@floor), max(@floor));
print "\n";
if ($clo > $fhi) {
  printf "分離した。帯は %.3f〜%.3f（隙間 %.3f）\n", $fhi, $clo, $clo - $fhi;
} else {
  my ($lo, $hi) = (max($clo,$flo), min($chi,$fhi));
  my $ov = $hi - $lo;
  my $cs = $chi - $clo || 1e-12;
  my $fs = $fhi - $flo || 1e-12;
  printf "重なった。重なり %.3f（天井の %.0f%%、床の %.0f%%）%s\n",
         $ov, $ov/$cs*100, $ov/$fs*100,
         (max($ov/$cs, $ov/$fs) > 0.5) ? '  ← 5 割超。目盛りを作らない' : '';
}

# ---- 検める ----------------------------------------------------------
# 目盛りは上で固定した。ここから先は測るだけで、当てはめ直さない。
if (@check) {
  printf "\n%-34s %-9s %s\n", '検める文', '照合値', '位置';
  printf "%s\n", '-' x 60;
  for my $g (@check) {
    for my $p (sort glob $g) {
      my $f = features(prose($p));
      (my $n = $p) =~ s{.*/}{};
      my $u = { name => "検:$n", f => $f };
      project($u);
      my $v = matching_value($u);
      my $where = $v >= $clo ? '天井の中'
                : $v <= $fhi ? '床の側'
                :              '帯の中';
      # 系統ごとの対数尤度比も出す。合算値が動かないとき、
      # 系統が動いていないのか重みが小さいのかを分けるため。
      my (@per, @dist);
      for my $s (@systems) {
        my @d = map { cosd($vec{$s->{name}}{$u->{name}}, $vec{$s->{name}}{$_->{name}}) } @partners;
        push @dist, median(@d);
        push @per, median(map { apply_logistic($sysw{$s->{name}}, [$_]) } @d);
      }
      printf "%-34s %+9.3f %s\n", substr($n, 0, 33), $v, $where;
      printf "%-34s %s\n", '  距離',
             join '  ', map { sprintf '%s %.4f', $systems[$_]{name}, $dist[$_] } 0 .. $#systems;
      printf "%-34s %s\n", '  尤度比',
             join '  ', map { sprintf '%s %+.3f', $systems[$_]{name}, $per[$_] } 0 .. $#systems;
    }
  }
  printf "%s\n", '-' x 60;
  printf "床の上端 %+.3f   天井の下端 %+.3f\n", $fhi, $clo;
}

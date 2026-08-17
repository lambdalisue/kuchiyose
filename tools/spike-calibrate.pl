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
my (%drop, $rotate);
while (@opt) {
  my $k = shift @opt;
  if    ($k eq '--drop')   { my $v = shift @opt; $drop{decode_utf8($v)} = 1 if defined $v }
  elsif ($k eq '--rotate') { $rotate = shift @opt }
}

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
    $l =~ s/^\s*\|\s*//; $l =~ s/\s*\|\s*$//;
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
      $ty{ $c =~ /\p{Hiragana}/          ? 'hira'
         : $c =~ /\p{Katakana}|\x{30FC}/ ? 'kata'
         : $c =~ /\p{Han}|\x{3005}/      ? 'kanji'
         : $c =~ /[A-Za-z]/              ? 'alpha'
         : $c =~ /[0-9]/                 ? 'digit'
         : $c =~ /[。、！？!?「」『』（）()・…〜～]/ ? 'punct'
         : $c =~ /\s/                    ? 'space' : 'other' }++;
      $ja++ if $c =~ /[\p{Hiragana}\p{Katakana}\p{Han}\x{30FC}\x{3005}]/;
    }
  }
  return { bi => \%bi, before => \%bf, after => \%af, types => \%ty, ja => $ja || 0 };
}

sub load {
  my @u;
  for my $p (sort glob shift) {
    my $f = features(prose($p));
    next if $f->{ja} < 1000;
    (my $n = $p) =~ s{.*/}{};
    push @u, { name => $n, f => $f };
  }
  return @u;
}

my @person = load($pg);
my @base   = load($bg);
die "本人が 6 単位に届かない\n" if @person < 6;
die "基準が 4 単位に届かない\n" if @base < 4;

my @systems = grep { !$drop{$_->{name}} } (
  { name => '文字bigram', key => 'bi',     n => 500 },
  { name => '読点前',      key => 'before', n => 50  },
  { name => '読点後',      key => 'after',  n => 50  },
  { name => '文字種',      key => 'types',  n => 10  },
);

# ---- 語彙と z 得点は割る前に 1 度だけ --------------------------------
my @all = (@person, @base);
my %vec;
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
  $vec{$s->{name}}{$all[$_]{name}} =
    [ map { $sd[$_] > 0 ? ($raw[0][$_]) : 0 } 0 .. -1 ] for ();   # placeholder
  for my $i (0 .. $#all) {
    $vec{$s->{name}}{$all[$i]{name}} =
      [ map { $sd[$_] > 0 ? ($raw[$i][$_] - $mu[$_]) / $sd[$_] : 0 } 0 .. $d - 1 ];
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

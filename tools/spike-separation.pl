#!/usr/bin/env perl
# 骨格のスパイク——正規化 → 3 系統 → Cosine Delta → 距離の分布。
#
# 何を答えるか
#   同じ場面どうしの距離が、違う場面どうしの距離より小さく出るか。
#   出なければ、実装か指標のどちらかが壊れている。
#
# 何を答えないか
#   天井と床が分離するか。それには人の書いた日本語が要る。
#   ここで比べているのは、どちらも LLM が書いた文章の、場面の違いである。
use strict; use warnings;
use utf8; use open qw(:std :utf8);
use List::Util qw(sum min max);

my @groups = @ARGV;
die "usage: spike-separation.pl <label>=<glob> ...\n" unless @groups >= 2;

# ---- 取り込み（最小の正規化）----------------------------------------
# 地の文だけを node ごとに切って返す。コードブロックとインラインコードは外す。
sub prose {
  my $path = shift;
  open my $h, '<:utf8', $path or die "$path: $!";
  my (@out, $fence);
  while (my $l = <$h>) {
    chomp $l;
    if ($l =~ /^\s*```/) { $fence = !$fence; next }
    next if $fence;
    next if $l =~ /^\s*$/;
    next if $l =~ /^\s*\|?\s*-{3,}/;          # 表の区切り
    $l =~ s/`[^`]*`//g;                        # インラインコード
    $l =~ s/<[^>]+>//g;                        # HTML タグ（強調など）
    $l =~ s/\[([^\]]*)\]\([^)]*\)/$1/g;        # リンクは文字だけ残す
    $l =~ s/^\s*[>#*\-]+\s*//;                 # 記法の頭
    $l =~ s/^\s*\d+\.\s*//;
    $l =~ s/^\s*\|\s*//; $l =~ s/\s*\|\s*$//;  # 表のセル
    next if $l =~ /^\s*$/;
    push @out, $l;
  }
  close $h;
  return \@out;
}

sub is_ja { my $c = shift; return $c =~ /[\p{Hiragana}\p{Katakana}\p{Han}\x{30FC}\x{3005}]/ }

# ---- 系統（解析器を要らない 3 つ）-----------------------------------
# 1. 文字 bigram   2. 読点の打ち方（直前・直後の文字）   3. 文字種
sub features {
  my $lines = shift;
  my (%bi, %before, %after, %types, $ja);
  for my $l (@$lines) {
    my @c = split //, $l;
    for my $i (0 .. $#c) {
      my $c = $c[$i];
      $bi{"$c[$i]$c[$i+1]"}++ if $i < $#c;    # node を跨がない
      if ($c eq '、') {
        $before{$i > 0 ? $c[$i-1] : '^'}++;
        $after{$i < $#c ? $c[$i+1] : '$'}++;
      }
      my $t = $c =~ /\p{Hiragana}/            ? 'hira'
            : $c =~ /\p{Katakana}|\x{30FC}/   ? 'kata'
            : $c =~ /\p{Han}|\x{3005}/        ? 'kanji'
            : $c =~ /[A-Za-z]/                ? 'alpha'
            : $c =~ /[0-9]/                   ? 'digit'
            : $c =~ /[。、！？!?「」『』（）()・…〜～]/ ? 'punct'
            : $c =~ /\s/                      ? 'space'
            :                                   'other';
      $types{$t}++;
      $ja++ if is_ja($c);
    }
  }
  return { bi => \%bi, before => \%before, after => \%after, types => \%types, ja => $ja || 0 };
}

# ---- 語彙を先に固定する（全体から 1 度だけ）-------------------------
sub fix_vocab {
  my ($feats, $key, $n) = @_;
  my %tot;
  for my $f (@$feats) { $tot{$_} += $f->{$key}{$_} for keys %{$f->{$key}} }
  my @v = sort { $tot{$b} <=> $tot{$a} || $a cmp $b } keys %tot;   # 同順位は昇順
  @v = @v[0 .. min($n, scalar @v) - 1];
  return \@v;
}

sub vectorize {
  my ($f, $key, $vocab) = @_;
  my $tot = sum(values %{$f->{$key}}) || 1;     # 分母は選ぶ前の全体
  return [ map { ($f->{$key}{$_} // 0) / $tot } @$vocab ];
}

# ---- Cosine Delta ----------------------------------------------------
# 次元ごとに z 得点 → コサイン距離（正規化はコサインに含まれる）
sub zscore {
  my $vecs = shift;
  my $d = scalar @{$vecs->[0]};
  my @mu, my @sd;
  for my $j (0 .. $d - 1) {
    my @col = map { $_->[$j] } @$vecs;
    my $m = sum(@col) / @col;
    my $v = sum(map { ($_ - $m) ** 2 } @col) / @col;
    push @mu, $m; push @sd, sqrt($v);
  }
  return [ map { my $v = $_; [ map { $sd[$_] > 0 ? ($v->[$_] - $mu[$_]) / $sd[$_] : 0 } 0 .. $d - 1 ] } @$vecs ];
}

sub cosine_distance {
  my ($a, $b) = @_;
  my ($dot, $na, $nb) = (0, 0, 0);
  for my $i (0 .. $#$a) { $dot += $a->[$i] * $b->[$i]; $na += $a->[$i] ** 2; $nb += $b->[$i] ** 2 }
  return 1 if $na == 0 || $nb == 0;
  return 1 - $dot / (sqrt($na) * sqrt($nb));
}

# ---- 読み込み --------------------------------------------------------
my (@label, @path, @feat);
for my $g (@groups) {
  my ($name, $glob) = split /=/, $g, 2;
  for my $p (sort glob $glob) {
    my $lines = prose($p);
    my $f = features($lines);
    next if $f->{ja} < 1000;                     # 除外——日本語 1,000 字未満
    push @label, $name; push @path, $p; push @feat, $f;
  }
}
printf "単位 %d 本（日本語 1,000 字以上）\n", scalar @feat;
my %n; $n{$_}++ for @label;
printf "  %s: %d 本\n", $_, $n{$_} for sort keys %n;
die "\n単位が足りない\n" if @feat < 4;

# ---- 系統ごとに距離を出す --------------------------------------------
my @systems = (
  { name => '文字 bigram',   key => 'bi',     n => 500 },
  { name => '読点・直前',     key => 'before', n => 50  },
  { name => '読点・直後',     key => 'after',  n => 50  },
  { name => '文字種',         key => 'types',  n => 10  },
);

printf "\n%-14s %-10s %-10s %-10s %s\n", '系統', '同じ場面', '違う場面', '差', '重なり';
printf "%s\n", '-' x 62;

my %fused;
for my $s (@systems) {
  my $vocab = fix_vocab(\@feat, $s->{key}, $s->{n});
  my @vecs  = map { vectorize($_, $s->{key}, $vocab) } @feat;
  my $z     = zscore(\@vecs);

  my (@same, @diff);
  for my $i (0 .. $#$z) {
    for my $j ($i + 1 .. $#$z) {
      my $d = cosine_distance($z->[$i], $z->[$j]);
      push @{ $label[$i] eq $label[$j] ? \@same : \@diff }, $d;
      $fused{"$i,$j"} += $d;
    }
  }
  my $ms = sum(@same) / @same;
  my $md = sum(@diff) / @diff;
  my $ov = overlap(\@same, \@diff);
  printf "%-14s %-10.4f %-10.4f %+-10.4f %s\n", $s->{name}, $ms, $md, $md - $ms,
         $ov ? sprintf('%.0f%%', $ov * 100) : '無し';
}

# 合算（重みは等しく。較正はしていない）
{
  my (@same, @diff);
  for my $k (keys %fused) {
    my ($i, $j) = split /,/, $k;
    push @{ $label[$i] eq $label[$j] ? \@same : \@diff }, $fused{$k} / scalar @systems;
  }
  my $ms = sum(@same) / @same;
  my $md = sum(@diff) / @diff;
  my $ov = overlap(\@same, \@diff);
  printf "%s\n", '-' x 62;
  printf "%-14s %-10.4f %-10.4f %+-10.4f %s\n", '合算（等重み）', $ms, $md, $md - $ms,
         $ov ? sprintf('%.0f%%', $ov * 100) : '無し';
}

# 2 つの分布の重なり——広がりに対する割合。仕様の帯の規則と同じ読み方。
sub overlap {
  my ($a, $b) = @_;
  my ($alo, $ahi) = (min(@$a), max(@$a));
  my ($blo, $bhi) = (min(@$b), max(@$b));
  my $lo = max($alo, $blo);
  my $hi = min($ahi, $bhi);
  return 0 if $hi <= $lo;
  my $span_a = $ahi - $alo || 1e-12;
  my $span_b = $bhi - $blo || 1e-12;
  return max(($hi - $lo) / $span_a, ($hi - $lo) / $span_b);
}

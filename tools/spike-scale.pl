#!/usr/bin/env perl
# 天井と床を、仕様どおりの形で作る。
#
#   相手集合          本人の単位から、名前の昇順で 5 本
#   天井              残りの本人の単位 1 本 × 相手集合 の中央値。1 本につき 1 点
#   床                基準の単位 1 本 × 相手集合 の中央値。1 本につき 1 点
#   帯                2 つの分布の重なり
#
# 較正はしない。系統ごとの距離を等重みで平均するだけである。
# 尤度比に変えていないので、これは目盛りではなく「分離しそうか」の当たりである。
use strict; use warnings;
use utf8; use open qw(:std :utf8);
use List::Util qw(sum min max);
use Encode qw(decode_utf8);

my ($person_glob, $baseline_glob, @opt) = @ARGV;
die "usage: spike-scale.pl <本人の glob> <基準の glob> [--drop 系統名]\n"
  unless $person_glob && $baseline_glob;
my %drop;
while (@opt) {
  my $k = shift @opt;
  if ($k eq '--drop') {
    my $v = shift @opt;
    $drop{decode_utf8($v)} = 1 if defined $v;
  }
}

sub prose {
  my $path = shift;
  open my $h, '<:utf8', $path or die "$path: $!";
  my (@out, $fence, $front);
  while (my $l = <$h>) {
    chomp $l;
    if ($. == 1 && $l =~ /^---\s*$/) { $front = 1; next }
    if ($front) { $front = 0 if $l =~ /^---\s*$/; next }
    if ($l =~ /^\s*```/) { $fence = !$fence; next }
    next if $fence;
    next if $l =~ /^\s*$/;
    next if $l =~ /^\s*\|?\s*-{3,}/;
    $l =~ s/`[^`]*`//g;
    $l =~ s/<[^>]+>//g;
    $l =~ s/\[([^\]]*)\]\([^)]*\)/$1/g;
    $l =~ s/^\s*[>#*\-]+\s*//;
    $l =~ s/^\s*\d+\.\s*//;
    $l =~ s/^\s*\|\s*//; $l =~ s/\s*\|\s*$//;
    next if $l =~ /^\s*$/;
    push @out, $l;
  }
  close $h;
  return \@out;
}

sub features {
  my $lines = shift;
  my (%bi, %before, %after, %types, $ja);
  for my $l (@$lines) {
    my @c = split //, $l;
    for my $i (0 .. $#c) {
      my $c = $c[$i];
      $bi{"$c[$i]$c[$i+1]"}++ if $i < $#c;
      if ($c eq '、') {
        $before{$i > 0 ? $c[$i-1] : '^'}++;
        $after{$i < $#c ? $c[$i+1] : '$'}++;
      }
      my $t = $c =~ /\p{Hiragana}/           ? 'hira'
            : $c =~ /\p{Katakana}|\x{30FC}/  ? 'kata'
            : $c =~ /\p{Han}|\x{3005}/       ? 'kanji'
            : $c =~ /[A-Za-z]/               ? 'alpha'
            : $c =~ /[0-9]/                  ? 'digit'
            : $c =~ /[。、！？!?「」『』（）()・…〜～]/ ? 'punct'
            : $c =~ /\s/                     ? 'space'
            :                                  'other';
      $types{$t}++;
      $ja++ if $c =~ /[\p{Hiragana}\p{Katakana}\p{Han}\x{30FC}\x{3005}]/;
    }
  }
  return { bi => \%bi, before => \%before, after => \%after, types => \%types, ja => $ja || 0 };
}

sub load {
  my $glob = shift;
  my @u;
  for my $p (sort glob $glob) {
    my $f = features(prose($p));
    next if $f->{ja} < 1000;
    my $n = $p; $n =~ s{.*/}{};
    push @u, { name => $n, f => $f };
  }
  return @u;
}

my @person   = load($person_glob);
my @baseline = load($baseline_glob);
printf "本人 %d 単位、基準 %d 単位（日本語 1,000 字以上）\n", scalar @person, scalar @baseline;
die "\n本人が 6 単位に届かない（相手集合 5 + 天井の点 1）\n" if @person < 6;
die "\n基準が 1 単位も無い\n" unless @baseline;

# 相手集合——名前の昇順で 5 本。残りが天井の点。
my @partners = @person[0 .. 4];
my @ceiling  = @person[5 .. $#person];
printf "相手集合 %d 本 / 天井の点 %d 本 / 床の点 %d 本\n",
       scalar @partners, scalar @ceiling, scalar @baseline;

my @systems = grep { !$drop{$_->{name}} } (
  { name => '文字bigram', key => 'bi',     n => 500 },
  { name => '読点前',      key => 'before', n => 50  },
  { name => '読点後',      key => 'after',  n => 50  },
  { name => '文字種',      key => 'types',  n => 10  },
);
printf "系統 %s\n", join('・', map { $_->{name} } @systems);

# 語彙と z 得点は、割る前に全体から 1 度だけ決める。
my @all = (@person, @baseline);

sub fix_vocab {
  my ($key, $n) = @_;
  my %tot;
  for my $u (@all) { $tot{$_} += $u->{f}{$key}{$_} for keys %{$u->{f}{$key}} }
  my @v = sort { $tot{$b} <=> $tot{$a} || $a cmp $b } keys %tot;
  return [ @v[0 .. min($n, scalar @v) - 1] ];
}

my %vec;
for my $s (@systems) {
  my $vocab = fix_vocab($s->{key}, $s->{n});
  my @raw = map {
    my $u = $_;
    my $tot = sum(values %{$u->{f}{$s->{key}}}) || 1;
    [ map { ($u->{f}{$s->{key}}{$_} // 0) / $tot } @$vocab ];
  } @all;
  # 次元ごとに z 得点
  my $d = scalar @{$raw[0]};
  my (@mu, @sd);
  for my $j (0 .. $d - 1) {
    my @col = map { $_->[$j] } @raw;
    my $m = sum(@col) / @col;
    push @mu, $m;
    push @sd, sqrt(sum(map { ($_ - $m) ** 2 } @col) / @col);
  }
  for my $i (0 .. $#all) {
    $vec{$s->{name}}{$all[$i]{name}} =
      [ map { $sd[$_] > 0 ? ($raw[$i][$_] - $mu[$_]) / $sd[$_] : 0 } 0 .. $d - 1 ];
  }
}

sub cos_dist {
  my ($a, $b) = @_;
  my ($dot, $na, $nb) = (0, 0, 0);
  for my $i (0 .. $#$a) { $dot += $a->[$i]*$b->[$i]; $na += $a->[$i]**2; $nb += $b->[$i]**2 }
  return 1 if $na == 0 || $nb == 0;
  return 1 - $dot / (sqrt($na)*sqrt($nb));
}

sub median { my @s = sort { $a <=> $b } @_; my $n = @s;
             return $n % 2 ? $s[$n/2] : ($s[$n/2 - 1] + $s[$n/2]) / 2 }

# 1 本 × 相手集合 の中央値。系統ごとに出して等重みで平均する。
sub score {
  my $u = shift;
  my @per;
  for my $s (@systems) {
    my @d = map { cos_dist($vec{$s->{name}}{$u->{name}}, $vec{$s->{name}}{$_->{name}}) }
            grep { $_->{name} ne $u->{name} } @partners;
    push @per, median(@d);
  }
  return sum(@per) / @per;
}

my @ceil = map { score($_) } @ceiling;
my @floor = map { score($_) } @baseline;

printf "\n%-8s %-8s %-8s %-8s %s\n", '', '下端', '上端', '中央', '広がり';
printf "%s\n", '-' x 48;
report('天井', \@ceil);
report('床',   \@floor);

sub report {
  my ($name, $v) = @_;
  printf "%-8s %-8.4f %-8.4f %-8.4f %.4f\n",
         $name, min(@$v), max(@$v), median(@$v), max(@$v) - min(@$v);
}

# 帯——仕様の両端の規則。距離なので小さいほうが本人側である。
my ($clo, $chi) = (min(@ceil),  max(@ceil));
my ($flo, $fhi) = (min(@floor), max(@floor));
printf "\n";
if ($chi < $flo) {
  printf "<strong>分離した。</strong> 帯は %.4f〜%.4f（隙間）\n", $chi, $flo;
  printf "  天井の上端 %.4f < 床の下端 %.4f\n", $chi, $flo;
} else {
  my $lo = max($clo, $flo); my $hi = min($chi, $fhi);
  my $ov = $hi - $lo;
  my $cs = $chi - $clo || 1e-12;
  my $fs = $fhi - $flo || 1e-12;
  printf "<strong>重なった。</strong> 帯は %.4f〜%.4f（重なり %.4f）\n", $lo, $hi, $ov;
  printf "  天井の広がりに対して %.0f%%、床の広がりに対して %.0f%%\n", $ov/$cs*100, $ov/$fs*100;
  printf "  %s\n", (max($ov/$cs, $ov/$fs) > 0.5)
    ? '仕様の 5 割の線を超えている——目盛りを作らない'
    : '5 割以内——目盛りを作れる';
}

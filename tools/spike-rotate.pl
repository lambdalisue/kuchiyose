#!/usr/bin/env perl
# 相手集合を回して、分離が頑健かを見る。
# 1 通りだけで分離しても、その 5 本を選んだ運かもしれない。
use strict; use warnings;
use utf8; use open qw(:std :utf8);
use List::Util qw(sum min max);

my ($pg, $bg, @rest) = @ARGV;
die "usage: spike-rotate.pl <本人 glob> <基準 glob> [--drop 系統]...\n" unless $pg && $bg;
my $extra = join ' ', map { / / ? "'$_'" : $_ } @rest;

my (@gap, @ov);
printf "%-6s %-10s %s\n", '回転', '結果', '重み（切片 / 系統…）';
printf "%s\n", '-' x 74;
for my $r (0 .. 9) {
  my $cmd = "perl tools/spike-calibrate.pl '$pg' '$bg' --rotate $r $extra 2>&1";
  my $out = `$cmd`;
  my ($w) = $out =~ /^重み\s+(.*)$/m;
  $w //= '';
  if ($out =~ /分離した。帯は.*隙間 ([\d.]+)/) {
    push @gap, $1;
    printf "%-6s %-10s %s\n", $r, "分離 +$1", $w;
  } elsif ($out =~ /重なった。重なり ([\d.]+).*天井の (\d+)%/) {
    push @ov, $1;
    printf "%-6s %-10s %s\n", $r, "重なり $1", $w;
  } else {
    printf "%-6s %-10s\n", $r, '読めない';
  }
}
printf "%s\n", '-' x 74;
printf "分離 %d / 10", scalar @gap;
printf "  隙間 最小 %.3f 中央 %.3f 最大 %.3f",
       min(@gap), (sort { $a <=> $b } @gap)[$#gap/2], max(@gap) if @gap;
printf "  重なり %d 回", scalar @ov if @ov;
print "\n";

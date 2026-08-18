#!/usr/bin/env perl
# 指示できる指標の側を端まで通す。軸が在るのはここである。
#
#   1  指示できる指標を測る（安く測れるものだけ）
#   2  標本の長さの範囲が揃っているかを先に確かめる
#   3  条件 1（幅が狭い）と条件 2（離れている）で「効く」を決める
#   4  検める文を測り、外れている指標を 3 つ選ぶ
#   5  観測・直し方・普段 の 3 つを揃えて出す
#
# 照合値は見ない。ここは 3 段目である。
use strict; use warnings;
use utf8; use open qw(:std :utf8);
use List::Util qw(sum min max);

my ($pg, $bg, @opt) = @ARGV;
die "usage: spike-directive.pl <本人 glob> <基準 glob> [--check <glob>]...\n"
  unless $pg && $bg;
my (@check, @values);
while (@opt) {
  my $k = shift @opt;
  push @check,  shift @opt if $k eq '--check';
  push @values, shift @opt if $k eq '--values';
}

our $HIRA  = qr/[\x{3041}-\x{3096}\x{309D}-\x{309F}]/;
our $KATA  = qr/[\x{30A1}-\x{30FA}\x{30FC}-\x{30FF}\x{FF66}-\x{FF9F}\x{3005}]/;
our $KANJI = qr/[\x{2F00}-\x{2FDF}\x{3400}-\x{4DBF}\x{4E00}-\x{9FFF}\x{F900}-\x{FAFF}\x{20000}-\x{2FA1F}]/;
our $JA    = qr/$HIRA|$KATA|$KANJI/;
our $JA_PROSE = qr/[\x{3041}-\x{3096}\x{309F}\x{30A1}-\x{30FA}\x{30FF}\x{FF66}-\x{FF6F}\x{FF71}-\x{FF9F}]|$KANJI/;
our $YAKU  = qr/[。、！？!?「」『』（）()・…〜～]/;

# ---- 取り込み --------------------------------------------------------
# node ごとに 1 本。段落かどうかも一緒に持つ（段落だけを見る指標がある）。
sub nodes {
  my $p = shift;
  open my $h, '<:utf8', $p or die "$p: $!";
  my (@o, $fence, $front);
  while (my $l = <$h>) {
    chomp $l;
    if ($. == 1 && $l =~ /^---\s*$/) { $front = 1; next }
    if ($front) { $front = 0 if $l =~ /^---\s*$/; next }
    if ($l =~ /^\s*```/) { $fence = !$fence; next }
    next if $fence || $l =~ /^\s*$/ || $l =~ /^\s*\|?\s*-{3,}/;
    my $kind = $l =~ /^\s*#/          ? 'heading'
             : $l =~ /^\s*[*+-]\s/    ? 'item'
             : $l =~ /^\s*\d+\.\s/    ? 'item'
             : $l =~ /^\s*>/          ? 'quote'
             : $l =~ /\|/             ? 'cell'
             :                          'para';
    $l =~ s/`[^`]*`//g; $l =~ s/<[^>]+>//g;
    $l =~ s/\[([^\]]*)\]\([^)]*\)/$1/g;
    $l =~ s/^\s*[>#*+\-]+\s*//; $l =~ s/^\s*\d+\.\s*//;
    if ($kind eq 'cell') {
      for my $c (split /\|/, $l) {
        $c =~ s/^\s+//; $c =~ s/\s+$//;
        next if $c eq '' || $c !~ $JA_PROSE;
        push @o, { kind => 'cell', text => $c };
      }
      next;
    }
    next if $l =~ /^\s*$/;
    push @o, { kind => $kind, text => $l };
  }
  close $h; return \@o;
}

# 文に切る。。！？!? で切り、直後の閉じ括弧は同じ文に含める。… では切らない。
sub sentences {
  my @c = split //, shift;
  my (@s, $cur);
  $cur = '';
  my $i = 0;
  while ($i <= $#c) {
    $cur .= $c[$i];
    if ($c[$i] =~ /[。！？!?]/) {
      $i++;
      # 連続する終止符は同じ文の末尾に付ける
      while ($i <= $#c && $c[$i] =~ /[。！？!?]/) { $cur .= $c[$i]; $i++ }
      # 直後の閉じ括弧も同じ文に含める
      while ($i <= $#c && $c[$i] =~ /[」』）)"]/) { $cur .= $c[$i]; $i++ }
      push @s, $cur; $cur = '';
      next;
    }
    $i++;
  }
  push @s, $cur if $cur =~ /\S/;
  return grep { /$JA/ } @s;
}

# ---- 指示できる指標 --------------------------------------------------
sub measure {
  my $nd = shift;
  my $prose = join '', map { $_->{text} } @$nd;
  my $ja = () = $prose =~ /$JA/g;
  return undef if $ja < 1000;                       # 除外の既定

  my %v;
  my $per1k = sub { 1000 * $_[0] / $ja };

  $v{'全角括弧'}   = $per1k->(scalar(() = $prose =~ /（/g));
  $v{'半角括弧'}   = $per1k->(scalar(() = $prose =~ /\(/g));
  $v{'感嘆符'}     = $per1k->(scalar(() = $prose =~ /[!！]/g));
  $v{'疑問符'}     = $per1k->(scalar(() = $prose =~ /[?？]/g));
  $v{'三点リーダ'} = $per1k->(scalar(() = $prose =~ /…+|\.{3,}/g));

  # 中黒——前後がともにカタカナのものは複合語の区切りなので除く
  my $nakaguro = 0;
  my @pc = split //, $prose;
  for my $i (0 .. $#pc) {
    next unless $pc[$i] eq '・';
    my $prev = $i > 0     ? $pc[$i-1] : '';
    my $next = $i < $#pc  ? $pc[$i+1] : '';
    next if $prev =~ /$KATA/ && $next =~ /$KATA/;
    $nakaguro++;
  }
  $v{'中黒'} = $per1k->($nakaguro);

  # 和欧間スペース欠落——分母は隣接の機会。約物を挟むものは数えない
  my ($chance, $miss) = (0, 0);
  for my $i (0 .. $#pc - 1) {
    my ($a, $b) = ($pc[$i], $pc[$i+1]);
    next if $a =~ $YAKU || $b =~ $YAKU;
    if (($a =~ /$JA/ && $b =~ /[A-Za-z0-9]/) || ($a =~ /[A-Za-z0-9]/ && $b =~ /$JA/)) {
      $chance++; $miss++;
    } elsif ($a eq ' ' && $i > 0 && $i < $#pc) {
      my ($p, $n) = ($pc[$i-1], $pc[$i+1]);
      next if $p =~ $YAKU || $n =~ $YAKU;
      $chance++ if ($p =~ /$JA/ && $n =~ /[A-Za-z0-9]/) || ($p =~ /[A-Za-z0-9]/ && $n =~ /$JA/);
    }
  }
  $v{'和欧間スペース欠落'} = $chance ? $miss / $chance : undef;

  # 段落だけを見る 2 つ
  my @paras = grep { $_->{kind} eq 'para' } @$nd;
  if (@paras >= 10) {
    my @len = map { scalar(() = $_->{text} =~ /$JA/g) } @paras;
    my $m = sum(@len) / @len;
    my $sd = sqrt(sum(map { ($_ - $m) ** 2 } @len) / @len);
    $v{'段落長の変動係数'} = $m ? $sd / $m : undef;
    my $one = grep { scalar(sentences($_->{text})) == 1 } @paras;
    $v{'1文だけの段落の割合'} = $one / scalar(@paras);
  }
  return { v => \%v, ja => $ja };
}

sub load {
  my @u;
  for my $p (sort glob shift) {
    my $m = measure(nodes($p));
    next unless $m;
    (my $n = $p) =~ s{.*/}{};
    push @u, { name => $n, %$m };
  }
  return @u;
}

# 条件 3（指摘して動くか）は基準を要らない。指標・文・改稿・再測定だけである。
# だから長さの防護柵より先に置く。
if (@values) {
  my (@rows, @names);
  for my $g (@values) {
    for my $p (sort glob $g) {
      my $m = measure(nodes($p));
      (my $n = $p) =~ s{.*/}{};
      unless ($m) { print "$n: 日本語 1,000 字に届かない\n"; next }
      push @rows, { name => $n, %$m };
      @names = sort keys %{ { %{$m->{v}}, map { %{$_->{v}} } @rows } };
    }
  }
  die "測れる文が無い\n" unless @rows;
  printf "%-26s %s\n", '指標', join '', map { sprintf '%14s', substr($_->{name}, 0, 13) } @rows;
  printf "%s\n", '-' x (26 + 14 * @rows);
  for my $k (@names) {
    printf "%-26s %s\n", $k,
           join '', map { defined $_->{v}{$k} ? sprintf '%14.4f', $_->{v}{$k} : sprintf '%14s', '—' } @rows;
  }
  if (@rows >= 2) {
    printf "%s\n", '-' x (26 + 14 * @rows);
    printf "%-26s %s\n", '1 本目からの差',
           join '', map {
             my $a = $rows[0]{v}{$_}; my $b = $rows[$#rows]{v}{$_};
             (defined $a && defined $b) ? sprintf('%14.4f', $b - $a) : sprintf('%14s', '—');
           } ();
    for my $k (@names) {
      my ($a, $b) = ($rows[0]{v}{$k}, $rows[$#rows]{v}{$k});
      next unless defined $a && defined $b;
      next if abs($b - $a) < 1e-9;
      printf "  %-24s %+.4f → %+.4f  差 %+.4f\n", $k, $a, $b, $b - $a;
    }
  }
  exit 0;
}

my @person = load($pg);
my @base   = load($bg);
die "素材が足りない\n" if @person < 10 || @base < 10;

printf "本人 %d 単位  基準 %d 単位\n", scalar @person, scalar @base;
printf "本人の長さ  %s\n", join ' ', map { $_->{ja} } sort { $a->{ja} <=> $b->{ja} } @person;
printf "基準の長さ  %s\n", join ' ', map { $_->{ja} } sort { $a->{ja} <=> $b->{ja} } @base;

# ---- 2. 標本の長さの範囲を先に確かめる -------------------------------
my ($plo, $phi) = (min(map { $_->{ja} } @person), max(map { $_->{ja} } @person));
my ($blo, $bhi) = (min(map { $_->{ja} } @base),   max(map { $_->{ja} } @base));
my $ov = min($phi, $bhi) - max($plo, $blo);
$ov = 0 if $ov < 0;
printf "長さ  本人 %d〜%d  基準 %d〜%d  重なり %d（本人の %.0f%%、基準の %.0f%%）\n",
       $plo, $phi, $blo, $bhi, $ov,
       100 * $ov / ($phi - $plo || 1), 100 * $ov / ($bhi - $blo || 1);
if ($ov / ($phi - $plo || 1) < 0.5 || $ov / ($bhi - $blo || 1) < 0.5) {
  print "\n<< 長さの範囲が半分も重ならない。判定を出さない >>\n";
  exit 1;
}

# ---- 3. 効くかを決める -----------------------------------------------
my @names = sort keys %{ { map { %{$_->{v}} } (@person, @base) } };

sub span {
  my ($set, $k) = @_;
  my @x = grep { defined } map { $_->{v}{$k} } @$set;
  return undef if @x < 3;
  return [min(@x), max(@x)];
}

printf "\n%-24s %-19s %-19s %-6s %-8s %s\n",
       '指標', '本人の幅', '基準の幅', '狭い', '離れて', '効く';
printf "%s\n", '-' x 92;
my (%eff, %prange);
for my $k (@names) {
  my $p = span(\@person, $k);
  my $b = span(\@base, $k);
  next unless $p && $b;
  $prange{$k} = $p;
  my ($pw, $bw) = ($p->[1] - $p->[0], $b->[1] - $b->[0]);
  my $narrow = $bw > 0 ? ($pw <= $bw / 2) : 0;

  # 離れているか——交わらない / 含む / 端が重なる
  my $far;
  my $o = min($p->[1], $b->[1]) - max($p->[0], $b->[0]);
  if ($o < 0)                                        { $far = 1 }             # 交わらない
  elsif ($b->[0] <= $p->[0] && $p->[1] <= $b->[1])   { $far = 0 }             # 基準が含む
  elsif ($p->[0] <= $b->[0] && $b->[1] <= $p->[1])   { $far = 0 }             # 本人が含む
  else                                               { $far = $pw > 0 ? ($o <= $pw / 2) : 0 }

  $eff{$k} = 1 if $narrow && $far;
  printf "%-24s %8.3f〜%-8.3f %8.3f〜%-8.3f %-6s %-8s %s\n",
         $k, @$p, @$b, ($narrow ? 'o' : '-'), ($far ? 'o' : '-'),
         ($narrow && $far ? '<< 効く' : '');
}
printf "%s\n", '-' x 92;
printf "効く指標 %d 本\n", scalar keys %eff;

# ---- 4/5. 検める ------------------------------------------------------
for my $g (@check) {
  for my $p (sort glob $g) {
    my $m = measure(nodes($p));
    (my $n = $p) =~ s{.*/}{};
    unless ($m) { print "\n$n: 日本語 1,000 字に届かない\n"; next }
    print "\n", '=' x 92, "\n$n\n", '=' x 92, "\n";
    my @out;
    for my $k (sort keys %eff) {
      my $x = $m->{v}{$k};
      next unless defined $x;
      my ($lo, $hi) = @{$prange{$k}};
      next if $x >= $lo && $x <= $hi;                  # 幅の中なら指摘しない
      my $gap = $x < $lo ? $lo - $x : $x - $hi;
      my $w = ($hi - $lo) || 1e-9;
      push @out, { k => $k, x => $x, lo => $lo, hi => $hi,
                   dir => ($x < $lo ? '増やす' : '減らす'), by => $gap / $w };
    }
    @out = sort { $b->{by} <=> $a->{by} } @out;
    unless (@out) { print "効く指標はすべて幅の中。指摘なし\n"; next }
    my $n_out = @out;
    @out = @out[0 .. min(2, $#out)];                   # 枠は 3 本
    printf "外れている %d 本のうち %d 本を出す\n\n", $n_out, scalar @out;
    for my $o (@out) {
      printf "%s\n", $o->{k};
      printf "  観測  %.3f\n", $o->{x};
      printf "  普段  %.3f〜%.3f\n", $o->{lo}, $o->{hi};
      printf "  直し方 %s（幅の %.1f 本ぶん外）\n\n", $o->{dir}, $o->{by};
    }
  }
}

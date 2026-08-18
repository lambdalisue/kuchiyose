#!/usr/bin/env perl
# 仕様の機械的検査。
#   1. 撤回した記述が残っていないか（retracted strings）
#   2. 札が位置表どおりか（tag shape）
#   3. 系統名が実在するか
use strict; use warnings;
use utf8; use open qw(:std :utf8);
use Encode qw(decode_utf8);
use File::Find;

my $root = shift or die "usage: checkspec.pl <docs dir>\n";
my (@files, $bad);
find(sub { push @files, $File::Find::name if /\.md$/ }, $root);

# ---- 1. 撤回した記述 -------------------------------------------------
# 「この文字列が残っていたら、直し忘れ」の一覧。直すたびに足す。
my @retracted = (
    ['読点とその直後の文字',        '読点 bigram は直前'],
  ['読点とその直後の文字の bigram','読点 bigram は直前'],
  ['6 系統以上は悪くなる',        '3〜7 系統はほぼ横ばい'],
  ['6 つ目からは悪くなる',        '3〜7 系統はほぼ横ばい'],
  ['5 つが頂点',                 '3〜7 系統はほぼ横ばい'],
  ['単独の値は報告されていない',   '機能語 unigram は 0.63002'],
  ['技術記事とほぼ同じ長さと素材', '素材は Yahoo! ブログの日記'],
  ['当てはめ方を書いていない',     'fusing-lr §5.2 に書いてある'],
  ['560 次元',                   'Writeprints-static は 557 次元'],
  ['GPT-4o mini',               'Post-Editing は o4-mini'],
  ['出どころが名指しした系統',     '層は札から引く'],
  ['出どころは必ず系統に遡れる',   '層は札から引く'],
  ['10 分割交差検証。',           '岩崎 2018 の 78.99% は学習内の再分類'],
  ['柳 佳',                      '著者は柳 燁佳'],
  ['## 正解率（Random Forest, LOO）', '柳・金 2023 の 10 群は検証法が書かれていない'],
  ['個人文体の情報が含まれている」と結論している', '柳・金 2023 は「推測できる」'],
  ['金・樺島・村上 (1993, 1994)',  '1994 は読点ではない'],
  ['規範の層には個人差が無い',      '岩崎 2018 は個人ごとの値を出していない'],
  ['本文は無料では見つからなかった', '井上・山名 2012 は dbsj.org が公開'],
  ['したがって 2 次元である',      '語り性は著者自身が分析から外している'],
);
for my $f (@files) {
  open my $fh, '<:utf8', $f or next;
  my $ln = 0;
  while (my $l = <$fh>) {
    $ln++;
    for my $r (@retracted) {
      next unless index($l, $r->[0]) >= 0;
      printf "RETRACTED  %s:%d  「%s」 — %s\n", rel($f), $ln, $r->[0], $r->[1];
      $bad++;
    }
  }
  close $fh;
}

# ---- 2/3. 札の形と系統名 ---------------------------------------------
my $mdir = "$root/spec/metrics";
if (-d $mdir) {
  my %systems = map { $_ => 1 } qw(
    文字bigram 文字\ bigram 機能語 品詞bigram 品詞\ bigram 読点の打ち方 文字種
    文節パターン 文末表現 語の文体値 表記 構造 長さ 埋め込み なし
  );
  my %classes = map { $_ => 1 } qw(記号 表記 語 品詞 構造 長さ 埋め込み);
  my %dirs    = map { $_ => 1 } qw(上限 下限 両側);
  opendir my $dh, $mdir or die;
  for my $b (sort readdir $dh) {
    next unless $b =~ /\.md$/;
    my $f = decode_utf8($b);
    next if $f eq 'README.md';
    open my $fh, '<:utf8', "$mdir/$b" or next;
    my @l = <$fh>; close $fh;
    my $body = join '', @l;
    my $tag = $l[2] // ''; chomp $tag;
    $tag =~ s/。\s*$//;
    my @p = split m{ / }, $tag;
    my $kind = $p[0] // '';

    # 4. 実装に足りる形か。欠けていればコードが書けない。
    printf "DEF  %s  数え方が無い\n", $f          and $bad++ unless $body =~ /^## 数え方/m;
    printf "DEF  %s  照合なのに次元が無い\n", $f   and $bad++ if $kind eq '照合' && $body !~ /^## 次元/m;
    printf "DEF  %s  指示なのに直し方が無い\n", $f and $bad++ if $kind eq '指示' && $body !~ /^## 直し方/m;

    if ($kind eq '指示') {
      if (@p != 5) { printf "TAG  %s  指示 は 5 欄: 「%s」\n", $f, $tag; $bad++; next }
      my ($sys, $cls, $dir) = @p[1,2,3];
      printf "TAG  %s  未知の系統「%s」\n", $f, $sys        and $bad++ unless $systems{$sys};
      printf "TAG  %s  未知の分類「%s」\n", $f, $cls        and $bad++ unless $classes{$cls};
      printf "TAG  %s  未知の向き「%s」\n", $f, $dir        and $bad++ unless $dirs{$dir};
    } elsif ($kind eq '照合') {
      printf "TAG  %s  照合 は 2 欄: 「%s」\n", $f, $tag and $bad++ if @p != 2;
    } elsif ($kind eq '人らしさ') {
      printf "TAG  %s  人らしさ は 2 欄: 「%s」\n", $f, $tag and $bad++ if @p != 2;
    } else {
      printf "TAG  %s  種別が読めない: 「%s」\n", $f, $tag; $bad++;
    }
  }
  closedir $dh;
}

sub rel { my $p = shift; $p =~ s{^\Q$root\E/?}{}; return $p }
print $bad ? "\n$bad 件\n" : "clean\n";
exit($bad ? 1 : 0);

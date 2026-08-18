use strict; use warnings; use utf8;
use open ':std', ':encoding(UTF-8)';
use Encode qw(decode_utf8 encode_utf8);
use File::Basename qw(dirname basename);
use File::Spec;

my $root = shift or die "need root";

my @files;
sub walk {
    my $d = shift;
    opendir(my $dh, $d) or die "$d: $!";
    for my $e (sort readdir $dh) {
        next if $e =~ /^\./;
        my $p = "$d/$e";
        if (-d $p) { walk($p) } elsif ($e =~ /\.md$/) { push @files, $p }
    }
    closedir $dh;
}
walk($root);

# 見出しから錨を作る。GitHub と同じ規則にする。
#
# 空白は 1 つずつ `-` にする。まとめて 1 つにしてはいけない——記号を落とした
# あとに空白が 2 つ並ぶ見出し（`## corpus/ — 正規形`）で、GitHub が作る錨と
# 食い違い、**正しいリンクを壊れていると誤報する**。
sub slug {
    my $h = shift;
    $h =~ s/`//g; $h =~ s/<[^>]*>//g;
    $h = lc $h; $h =~ s/[^\w\s-]//g; $h =~ s/\s/-/g;
    return $h;
}

# 経路は 1 つの形に正す。canonpath は先頭の ./ を落とすので、
# 両側を同じ関数に通さないと `..` を含むリンクだけが全部壊れて見える。
sub norm {
    my $t = File::Spec->canonpath(shift);
    $t =~ s{^\./}{};
    # 先頭の区間にも `..` は来る（tools/../docs/…）。両方畳む。
    1 while $t =~ s{(?:^|(?<=/))[^/]+/\.\.(?:/|$)}{};
    return $t;
}

my %anchors;
for my $p (@files) {
    open(my $fh, '<:encoding(UTF-8)', $p) or die $!;
    while (<$fh>) { $anchors{ norm(decode_utf8($p)) . '#' . slug($1) } = 1 if /^#+\s+(.*?)\s*$/ }
    close $fh;
}

my $bad = 0; my $total = 0;
for my $p (@files) {
    my $dp = decode_utf8($p);
    open(my $fh, '<:encoding(UTF-8)', $p) or die $!;
    my $body = do { local $/; <$fh> };
    close $fh;
    while ($body =~ m{\]\(([^)\s#]*\.md)?(#[^)]*)?\)}g) {
        my ($file, $frag) = ($1, $2);
        next unless defined $file or defined $frag;
        $total++;
        my $target = norm(defined $file ? dirname($dp) . '/' . $file : $dp);
        unless (-f encode_utf8($target)) {
            print "MISSING FILE: $file  (in $dp)\n"; $bad++; next;
        }
        if (defined $frag) {
            unless (exists $anchors{"$target$frag"}) {
                print "BROKEN ANCHOR: $file$frag  (in $dp)\n"; $bad++;
            }
        }
    }
}
print $bad ? "\n$bad broken / $total links\n" : "all $total links ok\n";

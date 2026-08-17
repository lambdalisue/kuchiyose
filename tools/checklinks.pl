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

sub slug {
    my $h = shift;
    $h =~ s/`//g; $h =~ s/<[^>]*>//g;
    $h = lc $h; $h =~ s/[^\w\s-]//g; $h =~ s/\s+/-/g;
    return $h;
}

my %anchors;
for my $p (@files) {
    open(my $fh, '<:encoding(UTF-8)', $p) or die $!;
    while (<$fh>) { $anchors{ decode_utf8($p) . '#' . slug($1) } = 1 if /^#+\s+(.*?)\s*$/ }
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
        my $target = defined $file
            ? File::Spec->canonpath(dirname($dp) . '/' . $file)
            : $dp;
        $target =~ s{/[^/]+/\.\.}{}g while $target =~ m{/[^/]+/\.\.};
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

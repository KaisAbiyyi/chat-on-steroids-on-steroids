using CoSoS.Probe7PB;

var tests = 0;
void Check(bool condition, string message)
{
    tests++;
    if (!condition) throw new Exception("FAIL " + message);
}
var book = new LeaseBook();
var alpha = book.Elect("synthetic-A", "tab-1");
Check(book.Allows(alpha, "synthetic-A", "tab-1"), "correct owner");
Check(!book.Allows(alpha, "synthetic-B", "tab-1"), "profile boundary");
Check(!book.Allows(alpha, "synthetic-A", "tab-2"), "tab boundary");
var beta = book.Elect("synthetic-B", "tab-1");
Check(!book.Allows(beta, "synthetic-A", "tab-1"), "second profile not aliased");
var replacement = book.Elect("synthetic-A", "tab-1");
Check(!book.Allows(alpha, "synthetic-A", "tab-1"), "stale duplicate election");
Check(book.Allows(replacement, "synthetic-A", "tab-1"), "new leader");
Check(!book.Close(alpha), "stale close cannot revoke new leader");
Check(book.Close(replacement), "close active target");
Check(!book.Allows(replacement, "synthetic-A", "tab-1"), "closed target denied");
Check(!book.Close(replacement), "double close denied");
Check(book.Allows(beta, "synthetic-B", "tab-1"), "other profile unaffected");
foreach (var invalid in new[] { "", " ", new string('a', 101) })
{
    bool rejected = false;
    try { book.Elect(invalid, "x"); }
    catch (ArgumentException) { rejected = true; }
    Check(rejected, "invalid identifier rejected");
}
Check(Path.IsPathFullyQualified(ProbeOutput.Resolve(null)), "output root absolute");
Console.WriteLine($"PASS: {tests} cloud synthetic policy assertions (no native browser claims)");

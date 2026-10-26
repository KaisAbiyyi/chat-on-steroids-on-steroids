using CoSoS.Probe7L1.WindowsHost;
var clipboard="dgiklkfkllikcanfonkcabmbdfmgleag";
var pdf="mhjfbmdgcfjbbpaeojofohoefgiehjai";
var owned="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
int assertions=0;
void Check(bool pass,string name) { assertions++; if(!pass) throw new Exception(name); }
Check(NativeProfileExtensions.Accepts([],true,null),"empty fresh inventory");
Check(NativeProfileExtensions.Accepts([clipboard,pdf],true,null),"signed runtime components in fresh profile");
Check(NativeProfileExtensions.Accepts([clipboard,pdf,owned],true,owned),"retained fixture and runtime components");
Check(!NativeProfileExtensions.Accepts([clipboard,pdf],false,null),"unverified runtime cannot attest components");
Check(!NativeProfileExtensions.Accepts([owned],true,null),"unexpected unpacked extension denied");
Check(!NativeProfileExtensions.Accepts([clipboard,owned],true,null),"one valid component cannot hide foreign extension");
Check(!NativeProfileExtensions.Accepts([clipboard,clipboard],true,null),"duplicate component refused");
Check(!NativeProfileExtensions.Accepts([owned,owned],true,owned),"duplicate owned fixture refused");
Check(!NativeProfileExtensions.Accepts([clipboard,pdf],true,owned),"missing retained fixture refused");
Check(!NativeProfileExtensions.Accepts([clipboard,pdf,owned,"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"],true,owned),"extra retained extension refused");
Check(!NativeProfileExtensions.Accepts([clipboard.ToUpperInvariant()],true,null),"noncanonical id refused");
Check(!NativeProfileExtensions.Accepts(["Microsoft Clipboard Extension"],true,null),"display name never confers trust");
Check(!NativeProfileExtensions.Accepts([clipboard],true,clipboard),"component cannot impersonate fixture");
Check(!NativeProfileExtensions.Accepts([""],true,null),"blank id refused");
Check(!NativeProfileExtensions.Accepts([],false,null),"runtime proof required even for empty inventory");
Check(NativeProfileExtensions.MayRestoreOwnedFixture([clipboard,pdf],true,owned),"crash-lost source-owned fixture can be restored");
Check(!NativeProfileExtensions.MayRestoreOwnedFixture([clipboard,pdf,owned],true,owned),"already-installed fixture is not restored");
Check(!NativeProfileExtensions.MayRestoreOwnedFixture([clipboard,"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"],true,owned),"foreign inventory cannot trigger restoration");
Check(!NativeProfileExtensions.MayRestoreOwnedFixture([clipboard,pdf],false,owned),"unverified runtime cannot restore fixture");
if(args.Length==1) {
 Check(NativeProfileExtensions.VerifiedMicrosoftRuntime(args[0]),"actual installed signed runtime");
 Check(!NativeProfileExtensions.VerifiedMicrosoftRuntime(typeof(Program).Assembly.Location),"unsigned arbitrary executable cannot attest components");
}
Console.WriteLine($"Native profile extension policy: {assertions} PASS");

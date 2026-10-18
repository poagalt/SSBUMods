// effmerge <dump folder> <out.eff>
//
// EffectConverter v1.9's folder -> .eff path, unchanged, plus the steps it skips: re-saving the
// texture bank and the model bank, and a model list kept one-per-descriptor. Its TextureInfo.Write has the BNTX re-serialisation commented out, so it always
// writes the BASE file's original BNTX bytes and only rebuilds the descriptor table. A same-file
// round trip therefore looks perfect, but an archive merged from several donors comes out with
// descriptors naming textures its BNTX does not contain -- measured 2026-10-18: 74 textures named,
// the base's 133 in the bank, none of them ours. This does exactly what the commented code would:
// OrderTextures(), BntxFile.Save(), and the section size set to match.
using System.Reflection;
using EffectLibrary.EFT2;
using EffectLibrary.Tools;
using EffectLibrary;

class Program
{
    static int Main(string[] args)
    {
        if (args.Length != 2)
        {
            Console.Error.WriteLine("usage: effmerge <dump folder> <out.eff>");
            return 2;
        }
        string folder = args[0], output = args[1];
        var ptcl = new PtclFile(Path.Combine(folder, "Base.ptcl"));
        var merged = PtclFileCreator.FromFolder(ptcl, folder);

        // MODELS, ONE PER DESCRIPTOR. PrimitiveInfo.AddPrimitive adds a descriptor for every new
        // primitive ID but adds its MODEL only if no model of that NAME is present -- and the game
        // (and the dumper's TryGetModel) find a primitive's model by the descriptor's INDEX. In a
        // merge, two donors' same-named models with different IDs therefore slide every later model
        // one place: measured 2026-10-18, Cloud's sword flare came back drawing EF_cmn_white00.
        // So the list is rebuilt here with exactly one model per descriptor, a clashing name made
        // unique by its ID (the name is only the dictionary key; the index is what is looked up).
        var prims = merged.Primitives;
        prims.PrimDescTable.Descriptors.Clear();
        prims.ResFile.Models.Clear();
        var seen = new HashSet<ulong>();
        var names = new HashSet<string>();
        var pending = new List<(ulong id, BfresLibrary.Model model)>();
        foreach (var bfres in Directory.GetFiles(folder, "*.bfres", SearchOption.AllDirectories).OrderBy(x => x))
        {
            ulong id = ulong.Parse(Path.GetFileNameWithoutExtension(bfres));
            if (!seen.Add(id))
                continue;
            var model = new BfresLibrary.ResFile(bfres).Models[0];
            if (!names.Add(model.Name))
            {
                model.Name = $"{model.Name}_{id}";
                names.Add(model.Name);
            }
            pending.Add((id, model));
        }
        // IN NAME ORDER, because the BFRES model dictionary is written sorted by name: insertion
        // order came back reordered on reload, so descriptor i no longer matched model i. Adding
        // both in the order the file will hold them keeps the index the game looks up correct.
        foreach (var (id, model) in pending.OrderBy(p => p.model.Name, StringComparer.Ordinal))
            prims.AddPrimitive(id, model);
        if (prims.PrimDescTable.Descriptors.Count != prims.ResFile.Models.Count)
        {
            Console.Error.WriteLine($"model list misaligned: {prims.PrimDescTable.Descriptors.Count} descriptors, " +
                                    $"{prims.ResFile.Models.Count} models");
            return 1;
        }

        // AND THE MODEL BANK ITSELF. PrimitiveInfo.Write has `SaveBinary()` commented out exactly
        // like the textures' re-save, so without this the BASE file's BFRES is written and only the
        // descriptors are rebuilt -- every model lookup then lands in the wrong file's models.
        prims.SaveBinary();

        var tex = merged.Textures;
        tex.OrderTextures();
        var mem = new MemoryStream();
        tex.BntxFile.Save(mem);
        tex.BinaryData = mem.ToArray();
        // SectionBase.Header is internal; the commented-out code sets Header.Size, so this does too.
        var hf = typeof(SectionBase).GetField("Header", BindingFlags.NonPublic | BindingFlags.Instance);
        object header = hf.GetValue(tex);
        header.GetType().GetField("Size").SetValue(header, (uint)tex.BinaryData.Length);
        hf.SetValue(tex, header);

        var namco = new NamcoEffectFile(merged);
        namco.Import(Path.Combine(folder, "NamcoFile.json"));
        namco.Save(output);
        Console.WriteLine($"wrote {output}: {merged.EmitterList.EmitterSets.Count} emitter sets, " +
                          $"{tex.BntxFile.Textures.Count} textures in a {tex.BinaryData.Length} byte BNTX, " +
                          $"{prims.ResFile.Models.Count} models");
        return 0;
    }
}

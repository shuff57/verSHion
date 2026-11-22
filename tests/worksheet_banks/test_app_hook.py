"""Workflow regression only: a local stub is not a model-quality measurement."""
import http.server,json,os,subprocess,tempfile,threading,unittest
from pathlib import Path
from PIL import Image,ImageDraw

ROOT=Path(__file__).resolve().parents[2]
BINARY=ROOT/'target/debug/vershion'

@unittest.skipUnless(BINARY.exists(),'Build the native app first')
class ImageBankHookTests(unittest.TestCase):
    def test_image_import_persists_a_bank_and_trace_in_its_own_store(self):
        requests=[]
        package={'format':'queshion/ditto','formatVersion':'0.1.0','bank':{'name':'Local hook fixture','questions':[{'id':'q1','label':'1','type':'short-answer','stem':'$5/4-3/4$','suggestedAnswer':''}]},'exams':[{'name':'Worksheet','sections':[{'title':'','instructions':'Evaluate each expression.'}],'positions':[{'question':'q1','section':0}]}],'report':{'unconverted':[],'notes':[]}}
        class Handler(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                requests.append(json.loads(self.rfile.read(int(self.headers['Content-Length']))))
                body=json.dumps({'choices':[{'finish_reason':'stop','message':{'content':json.dumps(package)}}]}).encode()
                self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
            def log_message(self,*_):pass
        server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler)
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        try:
            with tempfile.TemporaryDirectory() as directory:
                out=Path(directory);store=out/'store';store.mkdir()
                page=out/'page.png';im=Image.new('RGB',(400,300),'white');ImageDraw.Draw(im).text((25,25),'1) Evaluate 5/4 - 3/4',fill='black');im.save(page)
                (store/'settings.json').write_text(json.dumps({'ai_provider':'custom','ai_base':f'http://127.0.0.1:{server.server_port}/v1','ai_model':'local-stub','ai_native_tools':True}))
                env={k:v for k,v in os.environ.items() if not k.startswith('VERSHION_') and not k.endswith('API_KEY')}
                env.update(VERSHION_STORE_DIR=str(store),VERSHION_IMPORT_IMAGE=str(page),VERSHION_IMPORT_TRACE=str(out/'trace.json'),VERSHION_BANK_NAME='Isolated worksheet hook',VERSHION_AI_PARALLEL='1')
                p=subprocess.run([str(BINARY)],env=env,capture_output=True,text=True,timeout=20)
                self.assertEqual(p.returncode,0,p.stderr)
                banks=json.loads((store/'banks.json').read_text());trace=json.loads((out/'trace.json').read_text())
                self.assertEqual(len(banks),1);self.assertEqual(banks[0]['name'],'Isolated worksheet hook')
                self.assertEqual(banks[0]['questions'][0]['stem'],'Evaluate each expression.\n\n$5/4-3/4$')
                self.assertEqual(banks[0]['questions'][0]['points'],1.0)
                self.assertEqual(trace['questions'],1);self.assertEqual(trace['pages'][0]['questions'][0]['points'],None)
                self.assertEqual(trace['pages'][0]['figure_runs'],[])
                self.assertEqual(len(requests),1)
                self.assertTrue(requests[0]['messages'][0]['content'][1]['image_url']['url'].startswith('data:image/png;base64,'))
                self.assertFalse((store/'ai-keys.json').exists())
        finally:
            server.shutdown();server.server_close();thread.join(timeout=3)

if __name__=='__main__':unittest.main()
